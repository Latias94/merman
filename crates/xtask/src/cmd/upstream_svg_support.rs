//! Shared process and generated-file support for upstream SVG tooling.

use crate::XtaskError;
use sha2::{Digest, Sha256};
use std::cmp::Ordering as CmpOrdering;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static CONTENT_ADDRESSED_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(windows)]
mod windows_process_tree {
    use std::collections::{HashMap, HashSet};
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        PROCESS_TERMINATE, TerminateProcess, WaitForSingleObject,
    };

    const MAX_REFRESH_PASSES: usize = 4;
    const PROCESS_TERMINATION_WAIT_MILLIS: u32 = 5_000;

    struct OwnedHandle(HANDLE);

    impl OwnedHandle {
        fn open(pid: u32, access: u32) -> Option<Self> {
            // SAFETY: OpenProcess validates the observed PID and access mask. A non-null returned
            // handle is uniquely owned by this value.
            let handle = unsafe { OpenProcess(access, 0, pid) };
            (!handle.is_null()).then_some(Self(handle))
        }

        fn creation_time(&self) -> Option<u64> {
            let mut creation_time = FILETIME::default();
            let mut exit_time = FILETIME::default();
            let mut kernel_time = FILETIME::default();
            let mut user_time = FILETIME::default();
            // SAFETY: self contains a valid process handle and every FILETIME points to writable
            // storage for the duration of this call.
            if unsafe {
                GetProcessTimes(
                    self.0,
                    &mut creation_time,
                    &mut exit_time,
                    &mut kernel_time,
                    &mut user_time,
                )
            } == 0
            {
                return None;
            }
            Some(
                (u64::from(creation_time.dwHighDateTime) << 32)
                    | u64::from(creation_time.dwLowDateTime),
            )
        }
    }

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: OwnedHandle is constructed only from a valid, uniquely owned Win32 handle.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    struct ProcessIdentity {
        pid: u32,
        creation_time: u64,
    }

    #[derive(Clone, Copy, Debug)]
    struct ProcessSnapshot {
        identity: ProcessIdentity,
        parent_pid: u32,
    }

    struct ProcessGraph {
        root: ProcessIdentity,
        depths: HashMap<u32, (ProcessIdentity, usize)>,
    }

    impl ProcessGraph {
        fn new(root: ProcessIdentity) -> Self {
            Self {
                root,
                depths: HashMap::from([(root.pid, (root, 0))]),
            }
        }

        fn discover_descendants(
            &mut self,
            processes: &[ProcessSnapshot],
            protected: &HashSet<ProcessIdentity>,
            mut retain: impl FnMut(ProcessIdentity) -> bool,
        ) -> Vec<(ProcessIdentity, usize)> {
            loop {
                let mut discovered = false;
                for process in processes {
                    let identity = process.identity;
                    if identity.pid == 0
                        || identity.pid == self.root.pid
                        || protected.contains(&identity)
                    {
                        continue;
                    }
                    if let Some((known_identity, _)) = self.depths.get(&identity.pid) {
                        // A retained handle prevents a known PID from being recycled while cleanup
                        // is active. A different identity must therefore be unrelated.
                        if *known_identity != identity {
                            continue;
                        }
                        continue;
                    }
                    let Some((parent_identity, parent_depth)) =
                        self.depths.get(&process.parent_pid).copied()
                    else {
                        continue;
                    };
                    // ToolHelp records creator PIDs, which can outlive their process and later refer
                    // to an unrelated process after PID reuse. A real child cannot predate its parent.
                    if identity.creation_time < parent_identity.creation_time || !retain(identity) {
                        continue;
                    }
                    self.depths
                        .insert(identity.pid, (identity, parent_depth + 1));
                    discovered = true;
                }
                if !discovered {
                    break;
                }
            }

            let mut descendants: Vec<_> = processes
                .iter()
                .filter_map(|process| {
                    let (known_identity, depth) =
                        self.depths.get(&process.identity.pid).copied()?;
                    (known_identity == process.identity && known_identity != self.root)
                        .then_some((known_identity, depth))
                })
                .collect();
            descendants.sort_unstable_by(|left, right| {
                right
                    .1
                    .cmp(&left.1)
                    .then_with(|| right.0.pid.cmp(&left.0.pid))
            });
            descendants
        }
    }

    pub(super) struct ProcessTree {
        graph: Option<ProcessGraph>,
        handles: HashMap<ProcessIdentity, OwnedHandle>,
        protected: HashSet<ProcessIdentity>,
        live_descendants: Vec<(ProcessIdentity, usize)>,
    }

    impl ProcessTree {
        pub(super) fn capture(root_pid: u32) -> Self {
            let Some(root_handle) = OwnedHandle::open(root_pid, PROCESS_QUERY_LIMITED_INFORMATION)
            else {
                return Self {
                    graph: None,
                    handles: HashMap::new(),
                    protected: HashSet::new(),
                    live_descendants: Vec::new(),
                };
            };
            let Some(creation_time) = root_handle.creation_time() else {
                return Self {
                    graph: None,
                    handles: HashMap::new(),
                    protected: HashSet::new(),
                    live_descendants: Vec::new(),
                };
            };
            let root = ProcessIdentity {
                pid: root_pid,
                creation_time,
            };
            let mut tree = Self {
                graph: Some(ProcessGraph::new(root)),
                handles: HashMap::from([(root, root_handle)]),
                protected: HashSet::new(),
                live_descendants: Vec::new(),
            };
            if let Some(processes) = snapshot_processes() {
                tree.protected = process_ancestry(&processes, std::process::id());
                tree.live_descendants = tree.discover_descendants(&processes);
            }
            tree
        }

        pub(super) fn terminate_descendants(mut self) {
            for _ in 0..MAX_REFRESH_PASSES {
                self.terminate_processes();
                let Some(processes) = snapshot_processes() else {
                    return;
                };
                self.protected
                    .extend(process_ancestry(&processes, std::process::id()));
                self.live_descendants = self.discover_descendants(&processes);
                if self.live_descendants.is_empty() {
                    return;
                }
            }
            self.terminate_processes();
        }

        fn discover_descendants(
            &mut self,
            processes: &[ProcessSnapshot],
        ) -> Vec<(ProcessIdentity, usize)> {
            let Some(graph) = self.graph.as_mut() else {
                return Vec::new();
            };
            let handles = &mut self.handles;
            graph.discover_descendants(processes, &self.protected, |identity| {
                let Some(handle) = OwnedHandle::open(
                    identity.pid,
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                ) else {
                    return false;
                };
                if handle.creation_time() != Some(identity.creation_time) {
                    return false;
                }
                handles.insert(identity, handle);
                true
            })
        }

        fn terminate_processes(&self) {
            for &(identity, _) in &self.live_descendants {
                if self.protected.contains(&identity) {
                    continue;
                }
                let Some(process) = self.handles.get(&identity) else {
                    continue;
                };
                // SAFETY: process is the retained handle for the exact creation-time identity that
                // was classified as a descendant and was opened with PROCESS_TERMINATE access.
                let _ = unsafe { TerminateProcess(process.0, 1) };
            }
            for &(identity, _) in &self.live_descendants {
                if self.protected.contains(&identity) {
                    continue;
                }
                let Some(process) = self.handles.get(&identity) else {
                    continue;
                };
                // SAFETY: process is the retained handle for the exact descendant identity and
                // was opened with PROCESS_SYNCHRONIZE access. TerminateProcess is asynchronous for
                // another process, so wait until its handles, including listening sockets, close.
                let _ = unsafe { WaitForSingleObject(process.0, PROCESS_TERMINATION_WAIT_MILLIS) };
            }
        }
    }

    fn process_ancestry(processes: &[ProcessSnapshot], start_pid: u32) -> HashSet<ProcessIdentity> {
        let by_pid: HashMap<_, _> = processes
            .iter()
            .map(|process| (process.identity.pid, process))
            .collect();
        let mut ancestry = HashSet::new();
        let mut cursor = start_pid;
        while let Some(process) = by_pid.get(&cursor) {
            if !ancestry.insert(process.identity) || process.parent_pid == 0 {
                break;
            }
            cursor = process.parent_pid;
        }
        ancestry
    }

    fn snapshot_processes() -> Option<Vec<ProcessSnapshot>> {
        // SAFETY: the snapshot call has no pointer arguments and returns an owned handle.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let _snapshot = OwnedHandle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..PROCESSENTRY32W::default()
        };
        let mut processes = Vec::new();

        // SAFETY: entry points to a correctly sized, writable PROCESSENTRY32W value for the
        // lifetime of the snapshot enumeration.
        if unsafe { Process32FirstW(snapshot, &mut entry) } == 0 {
            return Some(processes);
        }
        loop {
            let pid = entry.th32ProcessID;
            if let Some(handle) = OwnedHandle::open(pid, PROCESS_QUERY_LIMITED_INFORMATION)
                && let Some(creation_time) = handle.creation_time()
            {
                processes.push(ProcessSnapshot {
                    identity: ProcessIdentity { pid, creation_time },
                    parent_pid: entry.th32ParentProcessID,
                });
            }
            // SAFETY: the same valid snapshot and writable entry are reused until enumeration ends.
            if unsafe { Process32NextW(snapshot, &mut entry) } == 0 {
                break;
            }
        }
        Some(processes)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn process(pid: u32, parent_pid: u32, creation_time: u64) -> ProcessSnapshot {
            ProcessSnapshot {
                identity: ProcessIdentity { pid, creation_time },
                parent_pid,
            }
        }

        #[test]
        fn process_graph_ignores_stale_parent_ids_from_before_root_creation() {
            let root = ProcessIdentity {
                pid: 100,
                creation_time: 200,
            };
            let mut graph = ProcessGraph::new(root);
            let processes = [
                process(100, 10, 200),
                process(200, 100, 150),
                process(300, 100, 210),
                process(400, 300, 220),
            ];

            let descendants = graph.discover_descendants(&processes, &HashSet::new(), |_| true);

            assert_eq!(
                descendants,
                vec![(processes[3].identity, 2), (processes[2].identity, 1)]
            );
        }

        #[test]
        fn process_graph_never_discovers_a_protected_caller_ancestor() {
            let root = ProcessIdentity {
                pid: 100,
                creation_time: 200,
            };
            let mut graph = ProcessGraph::new(root);
            let processes = [
                process(100, 10, 200),
                process(300, 100, 210),
                process(400, 300, 220),
            ];
            let protected = HashSet::from([processes[1].identity]);

            let descendants = graph.discover_descendants(&processes, &protected, |_| true);

            assert!(descendants.is_empty());
        }

        #[test]
        fn process_ancestry_includes_the_caller_and_each_live_ancestor() {
            let processes = [
                process(100, 0, 100),
                process(200, 100, 200),
                process(300, 200, 300),
                process(400, 100, 400),
            ];

            let ancestry = process_ancestry(&processes, 300);

            assert_eq!(
                ancestry,
                HashSet::from([
                    processes[0].identity,
                    processes[1].identity,
                    processes[2].identity,
                ])
            );
        }
    }
}

#[derive(Debug)]
struct PackageTreeEntry {
    relative_path: String,
    full_path: PathBuf,
}

fn compare_js_strings(left: &str, right: &str) -> CmpOrdering {
    left.encode_utf16().cmp(right.encode_utf16())
}

fn collect_package_tree_entries(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<PackageTreeEntry>,
) -> Result<(), XtaskError> {
    let children = fs::read_dir(directory).map_err(|source| XtaskError::ReadFile {
        path: directory.display().to_string(),
        source,
    })?;
    for child in children {
        let child = child.map_err(|source| XtaskError::ReadFile {
            path: directory.display().to_string(),
            source,
        })?;
        let full_path = child.path();
        let file_type = child.file_type().map_err(|source| XtaskError::ReadFile {
            path: full_path.display().to_string(),
            source,
        })?;
        if file_type.is_dir() {
            if child.file_name() == "node_modules" {
                continue;
            }
            collect_package_tree_entries(root, &full_path, entries)?;
            continue;
        }
        if !file_type.is_file() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "unsupported filesystem entry in upstream SVG runtime package: {}",
                full_path.display()
            )));
        }
        let relative = full_path.strip_prefix(root).map_err(|_| {
            XtaskError::UpstreamSvgFailed(format!(
                "upstream SVG runtime package entry escaped its root {}: {}",
                root.display(),
                full_path.display()
            ))
        })?;
        let mut components = Vec::new();
        for component in relative.components() {
            let std::path::Component::Normal(component) = component else {
                return Err(XtaskError::UpstreamSvgFailed(format!(
                    "invalid relative path in upstream SVG runtime package: {}",
                    relative.display()
                )));
            };
            let component = component.to_str().ok_or_else(|| {
                XtaskError::UpstreamSvgFailed(format!(
                    "non-Unicode path in upstream SVG runtime package: {}",
                    full_path.display()
                ))
            })?;
            components.push(component);
        }
        entries.push(PackageTreeEntry {
            relative_path: components.join("/"),
            full_path,
        });
    }
    Ok(())
}

pub(crate) fn upstream_svg_package_tree_sha256(root: &Path) -> Result<String, XtaskError> {
    if !root.is_dir() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG runtime package root is not a directory: {}",
            root.display()
        )));
    }
    let mut entries = Vec::new();
    collect_package_tree_entries(root, root, &mut entries)?;
    entries.sort_by(|left, right| compare_js_strings(&left.relative_path, &right.relative_path));

    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    for entry in entries {
        hash.update(entry.relative_path.as_bytes());
        hash.update([0]);
        let mut file = fs::File::open(&entry.full_path).map_err(|source| XtaskError::ReadFile {
            path: entry.full_path.display().to_string(),
            source,
        })?;
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|source| XtaskError::ReadFile {
                    path: entry.full_path.display().to_string(),
                    source,
                })?;
            if read == 0 {
                break;
            }
            hash.update(&buffer[..read]);
        }
        hash.update([0]);
    }
    let digest = hash.finalize();
    Ok(format!("{digest:x}"))
}

pub(crate) fn spawn_timeout_managed_child(command: &mut Command) -> std::io::Result<Child> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command.spawn()
}

struct CapturedProcessTree {
    #[cfg(unix)]
    process_group: Option<libc::pid_t>,
    #[cfg(windows)]
    process_tree: windows_process_tree::ProcessTree,
}

impl CapturedProcessTree {
    fn capture(child: &Child) -> Self {
        Self {
            #[cfg(unix)]
            process_group: libc::pid_t::try_from(child.id())
                .ok()
                .filter(|process_group| *process_group > 0),
            #[cfg(windows)]
            process_tree: windows_process_tree::ProcessTree::capture(child.id()),
        }
    }

    fn terminate(self, child: &mut Child) {
        #[cfg(unix)]
        if let Some(process_group) = self.process_group {
            // SAFETY: the positive PGID is converted without truncation from a child created by
            // spawn_timeout_managed_child with process_group(0). Negating it therefore targets
            // only that child's process group, never kill(0, ...) or kill(-1, ...).
            let _ = unsafe { libc::kill(-process_group, libc::SIGKILL) };
        }
        let _ = child.kill();
        #[cfg(windows)]
        self.process_tree.terminate_descendants();
        let _ = child.wait();
    }
}

pub(crate) fn terminate_child_process_tree(child: &mut Child) {
    CapturedProcessTree::capture(child).terminate(child);
}

fn terminate_captured_process_tree(
    child: &mut Child,
    process_tree: &mut Option<CapturedProcessTree>,
) {
    if let Some(process_tree) = process_tree.take() {
        process_tree.terminate(child);
    } else {
        terminate_child_process_tree(child);
    }
}

fn remaining_timeout(started: Instant, timeout: Duration) -> Option<Duration> {
    timeout
        .checked_sub(started.elapsed())
        .filter(|value| !value.is_zero())
}

fn timeout_error(context: &str) -> XtaskError {
    XtaskError::UpstreamSvgFailed(format!(
        "upstream SVG render environment probe failed: process timed out {context}"
    ))
}

fn receive_child_pipe(
    receiver: &Receiver<Result<Vec<u8>, XtaskError>>,
    description: &str,
    started: Instant,
    timeout: Duration,
) -> Result<Vec<u8>, XtaskError> {
    let Some(remaining) = remaining_timeout(started, timeout) else {
        return Err(timeout_error(&format!("while draining {description}")));
    };
    match receiver.recv_timeout(remaining) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            Err(timeout_error(&format!("while draining {description}")))
        }
        Err(RecvTimeoutError::Disconnected) => Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render probe {description} reader panicked"
        ))),
    }
}

fn spawn_child_pipe_reader(
    pipe: impl Read + Send + 'static,
    description: &'static str,
    max_bytes: u64,
) -> std::io::Result<Receiver<Result<Vec<u8>, XtaskError>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = std::thread::Builder::new()
        .name(format!("upstream-svg-probe-{description}"))
        .spawn(move || {
            let result = read_bounded_child_pipe(pipe, description, max_bytes);
            let _ = sender.send(result);
        })?;
    drop(reader);
    Ok(receiver)
}

fn wait_for_child(child: &mut Child, timeout: Duration) -> std::io::Result<ExitStatus> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(err) => return Err(err),
        }
        let Some(remaining) = remaining_timeout(start, timeout) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "process timed out",
            ));
        };
        std::thread::sleep(remaining.min(Duration::from_millis(25)));
    }
}

pub(crate) fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> std::io::Result<ExitStatus> {
    let status = wait_for_child(child, timeout);
    if status.is_err() {
        terminate_child_process_tree(child);
    }
    status
}

pub(crate) fn read_bounded_child_pipe(
    mut pipe: impl Read,
    description: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, XtaskError> {
    let max_bytes = usize::try_from(max_bytes).map_err(|_| {
        XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render probe {description} byte limit is too large for this platform"
        ))
    })?;
    let mut bytes = Vec::with_capacity(max_bytes.min(16 * 1024));
    let mut buffer = [0u8; 16 * 1024];
    let mut exceeded = false;
    loop {
        let read = pipe.read(&mut buffer).map_err(|err| {
            XtaskError::UpstreamSvgFailed(format!(
                "failed to read upstream SVG render probe {description}: {err}"
            ))
        })?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(bytes.len());
        let retained = remaining.min(read);
        bytes.extend_from_slice(&buffer[..retained]);
        exceeded |= retained < read;
    }
    if exceeded {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render probe {description} exceeded {max_bytes} bytes"
        )));
    }
    Ok(bytes)
}

pub(crate) fn wait_with_bounded_output(
    child: &mut Child,
    timeout: Duration,
    max_bytes_per_pipe: u64,
) -> Result<Output, XtaskError> {
    let started = Instant::now();
    let mut process_tree = Some(CapturedProcessTree::capture(child));
    let stdout = child.stdout.take().ok_or_else(|| {
        terminate_captured_process_tree(child, &mut process_tree);
        XtaskError::UpstreamSvgFailed(
            "upstream SVG render probe stdout was not captured".to_string(),
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        terminate_captured_process_tree(child, &mut process_tree);
        XtaskError::UpstreamSvgFailed(
            "upstream SVG render probe stderr was not captured".to_string(),
        )
    })?;

    let stdout_receiver =
        spawn_child_pipe_reader(stdout, "stdout", max_bytes_per_pipe).map_err(|err| {
            terminate_captured_process_tree(child, &mut process_tree);
            XtaskError::UpstreamSvgFailed(format!(
                "failed to start upstream SVG render probe stdout reader: {err}"
            ))
        })?;
    let stderr_receiver =
        spawn_child_pipe_reader(stderr, "stderr", max_bytes_per_pipe).map_err(|err| {
            terminate_captured_process_tree(child, &mut process_tree);
            XtaskError::UpstreamSvgFailed(format!(
                "failed to start upstream SVG render probe stderr reader: {err}"
            ))
        })?;

    let status = remaining_timeout(started, timeout)
        .ok_or_else(|| timeout_error("before the child wait started"))
        .and_then(|remaining| {
            wait_for_child(child, remaining).map_err(|err| {
                XtaskError::UpstreamSvgFailed(format!(
                    "upstream SVG render environment probe failed: {err}"
                ))
            })
        });
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            terminate_captured_process_tree(child, &mut process_tree);
            return Err(error);
        }
    };
    let stdout = match receive_child_pipe(&stdout_receiver, "stdout", started, timeout) {
        Ok(stdout) => stdout,
        Err(error) => {
            terminate_captured_process_tree(child, &mut process_tree);
            return Err(error);
        }
    };
    let stderr = match receive_child_pipe(&stderr_receiver, "stderr", started, timeout) {
        Ok(stderr) => stderr,
        Err(error) => {
            terminate_captured_process_tree(child, &mut process_tree);
            return Err(error);
        }
    };

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

pub(crate) fn ensure_content_addressed_file(
    dir: &Path,
    stem: &str,
    extension: &str,
    contents: &str,
) -> Result<PathBuf, XtaskError> {
    fs::create_dir_all(dir).map_err(|source| XtaskError::WriteFile {
        path: dir.display().to_string(),
        source,
    })?;
    let digest = Sha256::digest(contents.as_bytes());
    let file_path = dir.join(format!("{stem}-{digest:x}.{extension}"));
    match fs::read(&file_path) {
        Ok(existing) if existing == contents.as_bytes() => return Ok(file_path),
        Ok(_) => {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "content-addressed generated file is corrupted: {}",
                file_path.display()
            )));
        }
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            return Err(XtaskError::ReadFile {
                path: file_path.display().to_string(),
                source: err,
            });
        }
        Err(_) => {}
    }

    let sequence = CONTENT_ADDRESSED_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = dir.join(format!(
        ".{stem}.{}.{timestamp}.{sequence}.tmp.{extension}",
        std::process::id()
    ));
    fs::write(&temp_path, contents).map_err(|source| XtaskError::WriteFile {
        path: temp_path.display().to_string(),
        source,
    })?;

    match fs::rename(&temp_path, &file_path) {
        Ok(()) => Ok(file_path),
        Err(source) => {
            let concurrently_installed =
                fs::read(&file_path).is_ok_and(|existing| existing == contents.as_bytes());
            if let Err(err) = fs::remove_file(&temp_path) {
                eprintln!(
                    "warning: failed to remove generated file staging path {}: {err}",
                    temp_path.display()
                );
            }
            if concurrently_installed {
                Ok(file_path)
            } else {
                Err(XtaskError::WriteFile {
                    path: file_path.display().to_string(),
                    source,
                })
            }
        }
    }
}

pub(crate) fn ensure_content_addressed_js_script(
    dir: &Path,
    stem: &str,
    contents: &str,
) -> Result<PathBuf, XtaskError> {
    ensure_content_addressed_file(dir, stem, "js", contents)
}

pub(crate) fn ensure_upstream_svg_puppeteer_config() -> Result<PathBuf, XtaskError> {
    // Puppeteer 23.11.1 may not reliably propagate this option through every launcher path, so
    // the OS process-tree management above remains the authoritative timeout cleanup mechanism.
    const CONFIG: &str = "{\n  \"detached\": false\n}\n";
    ensure_content_addressed_file(
        &crate::cmd::target_root().join("xtask-js"),
        "upstream-svg-puppeteer-config",
        "json",
        CONFIG,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use std::net::{AddrParseError, SocketAddr, TcpListener};
    use std::process::Stdio;

    fn exact_test_name(name: &str) -> String {
        let name = format!("{}::{name}", module_path!());
        name.strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(name.as_str())
            .to_string()
    }

    fn unique_test_root(label: &str) -> PathBuf {
        let sequence = CONTENT_ADDRESSED_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        crate::cmd::target_root()
            .join("xtask-tests")
            .join(format!("{label}-{}-{sequence}", std::process::id()))
    }

    fn publish_ready_file_atomically(path: &Path, contents: &str) -> std::io::Result<()> {
        let parent = path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("readiness path has no parent: {}", path.display()),
            )
        })?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("ready");
        let sequence = CONTENT_ADDRESSED_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));

        // Keep the staging file beside the destination so rename publishes one complete payload
        // without exposing the empty/truncated write window that triggered the Windows failure.
        let result = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(contents.as_bytes())?;
            file.flush()?;
            drop(file);
            fs::rename(&temp_path, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    fn readiness_failure(
        child: &mut Child,
        process_tree: &mut Option<CapturedProcessTree>,
        error: std::io::Error,
    ) -> std::io::Error {
        terminate_captured_process_tree(child, process_tree);
        error
    }

    fn readiness_timeout_error(
        ready_path: &Path,
        last_parse_error: Option<&AddrParseError>,
    ) -> std::io::Error {
        let detail = last_parse_error.map_or_else(
            || "the readiness file was never published".to_string(),
            |err| format!("the last readiness payload was invalid: {err}"),
        );
        std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!(
                "timed out waiting for a valid listener address in {}: {detail}",
                ready_path.display()
            ),
        )
    }

    fn wait_for_managed_listener_address(
        child: &mut Child,
        ready_path: &Path,
        timeout: Duration,
    ) -> std::io::Result<SocketAddr> {
        let started = Instant::now();
        let mut process_tree = Some(CapturedProcessTree::capture(child));
        let mut last_parse_error = None;

        loop {
            if remaining_timeout(started, timeout).is_none() {
                let error = readiness_timeout_error(ready_path, last_parse_error.as_ref());
                return Err(readiness_failure(child, &mut process_tree, error));
            }

            match fs::read_to_string(ready_path) {
                Ok(contents) => match contents.trim().parse() {
                    Ok(address) => {
                        if remaining_timeout(started, timeout).is_some() {
                            return Ok(address);
                        }
                        let error = readiness_timeout_error(ready_path, last_parse_error.as_ref());
                        return Err(readiness_failure(child, &mut process_tree, error));
                    }
                    Err(err) => last_parse_error = Some(err),
                },
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(readiness_failure(child, &mut process_tree, err));
                }
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    let error = std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        format!(
                            "managed process exited with {status} before publishing a valid listener address to {}",
                            ready_path.display()
                        ),
                    );
                    return Err(readiness_failure(child, &mut process_tree, error));
                }
                Ok(None) => {}
                Err(err) => {
                    return Err(readiness_failure(child, &mut process_tree, err));
                }
            }

            let Some(remaining) = remaining_timeout(started, timeout) else {
                let error = readiness_timeout_error(ready_path, last_parse_error.as_ref());
                return Err(readiness_failure(child, &mut process_tree, error));
            };
            std::thread::sleep(remaining.min(Duration::from_millis(20)));
        }
    }

    fn wait_for_listener_release(address: SocketAddr) {
        let release_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match TcpListener::bind(address) {
                Ok(listener) => {
                    drop(listener);
                    return;
                }
                Err(_) if Instant::now() < release_deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => panic!("managed listener remained alive after cleanup: {err}"),
            }
        }
    }

    #[test]
    fn puppeteer_config_is_content_addressed_and_disables_detached_processes() {
        let path = ensure_upstream_svg_puppeteer_config().expect("install Puppeteer config");
        let contents = fs::read(&path).expect("read Puppeteer config");
        let config: serde_json::Value =
            serde_json::from_slice(&contents).expect("parse Puppeteer config");
        let digest = Sha256::digest(&contents);
        let expected_name = format!("upstream-svg-puppeteer-config-{digest:x}.json");

        assert_eq!(
            config.get("detached").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(expected_name.as_str())
        );
    }

    #[test]
    fn package_tree_hash_matches_the_javascript_protocol() {
        let root = unique_test_root("upstream-svg-package-tree");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("create package tree");
        fs::write(root.join("a.txt"), b"A").expect("write text package entry");
        fs::write(nested.join("b.bin"), [0, 0xff]).expect("write binary package entry");

        let digest = upstream_svg_package_tree_sha256(&root).expect("hash package tree");

        assert_eq!(
            digest,
            "3c2ace278ed1cba01db2b891cc80d1fa54e76032a0c594b8a722cabd2367b67a"
        );
        fs::remove_file(root.join("a.txt")).expect("remove text package entry");
        fs::remove_file(nested.join("b.bin")).expect("remove binary package entry");
        fs::remove_dir(&nested).expect("remove nested package directory");
        fs::remove_dir(&root).expect("remove package tree root");
    }

    #[cfg(unix)]
    #[test]
    fn package_tree_hash_ignores_nested_dependencies_but_rejects_other_symlinks() {
        use std::os::unix::fs::symlink;

        let root = unique_test_root("upstream-svg-package-tree-links");
        fs::create_dir_all(&root).expect("create package root");
        fs::write(root.join("index.js"), b"root package").expect("write package entry");
        let expected = upstream_svg_package_tree_sha256(&root).expect("hash package tree");

        let bin = root.join("node_modules/.bin");
        let package = root.join("node_modules/tool");
        fs::create_dir_all(&bin).expect("create package bin directory");
        fs::create_dir_all(&package).expect("create package directory");
        fs::write(package.join("cli.js"), b"cli").expect("write package executable");
        symlink("../tool/cli.js", bin.join("tool")).expect("create npm bin link");
        let with_nested_dependency =
            upstream_svg_package_tree_sha256(&root).expect("ignore nested dependency tree");
        assert_eq!(with_nested_dependency, expected);

        symlink("node_modules/tool/cli.js", root.join("unexpected-link"))
            .expect("create unsupported package link");
        let error = upstream_svg_package_tree_sha256(&root)
            .expect_err("non-bin package links must remain fail-closed");
        assert!(
            error.to_string().contains("unsupported filesystem entry"),
            "{error}"
        );

        fs::remove_file(root.join("unexpected-link")).expect("remove unsupported link");
        fs::remove_file(bin.join("tool")).expect("remove npm bin link");
        fs::remove_file(package.join("cli.js")).expect("remove package executable");
        fs::remove_dir(&package).expect("remove package directory");
        fs::remove_dir(&bin).expect("remove package bin directory");
        fs::remove_dir(root.join("node_modules")).expect("remove node_modules directory");
        fs::remove_file(root.join("index.js")).expect("remove package entry");
        fs::remove_dir(&root).expect("remove package tree root");
    }

    #[test]
    fn package_tree_paths_use_javascript_utf16_order() {
        assert_eq!(
            compare_js_strings("\u{10000}", "\u{e000}"),
            CmpOrdering::Less
        );
        assert_eq!("\u{10000}".cmp("\u{e000}"), CmpOrdering::Greater);
    }

    #[test]
    fn timeout_child_helper() {
        if std::env::var_os("MERMAN_XTASK_SUPPORT_TIMEOUT_CHILD").is_some() {
            std::thread::sleep(Duration::from_secs(30));
        }
    }

    #[test]
    fn large_pipe_child_helper() {
        if std::env::var_os("MERMAN_XTASK_SUPPORT_LARGE_PIPE_CHILD").is_none() {
            return;
        }

        const PAYLOAD_BYTES: usize = 512 * 1024;
        let stdout_writer = std::thread::spawn(|| {
            std::io::stdout()
                .lock()
                .write_all(&vec![b'o'; PAYLOAD_BYTES])
                .expect("write large stdout payload");
        });
        let stderr_writer = std::thread::spawn(|| {
            std::io::stderr()
                .lock()
                .write_all(&vec![b'e'; PAYLOAD_BYTES])
                .expect("write large stderr payload");
        });
        stdout_writer.join().expect("join stdout writer");
        stderr_writer.join().expect("join stderr writer");
    }

    #[test]
    fn inherited_pipe_grandchild_helper() {
        let Some(ready_path) =
            std::env::var_os("MERMAN_XTASK_SUPPORT_INHERITED_PIPE_GRANDCHILD_READY")
        else {
            return;
        };
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind inherited-pipe grandchild listener");
        publish_ready_file_atomically(
            Path::new(&ready_path),
            &listener
                .local_addr()
                .expect("inherited-pipe grandchild address")
                .to_string(),
        )
        .expect("write inherited-pipe ready file");
        std::thread::sleep(Duration::from_secs(30));
        drop(listener);
    }

    #[test]
    fn inherited_pipe_parent_helper() {
        let Some(ready_path) = std::env::var_os("MERMAN_XTASK_SUPPORT_INHERITED_PIPE_PARENT_READY")
        else {
            return;
        };
        let executable = std::env::current_exe().expect("current test executable");
        let grandchild_test = exact_test_name("inherited_pipe_grandchild_helper");
        let mut grandchild = Command::new(executable)
            .args(["--exact", grandchild_test.as_str(), "--nocapture"])
            .env(
                "MERMAN_XTASK_SUPPORT_INHERITED_PIPE_GRANDCHILD_READY",
                &ready_path,
            )
            .spawn()
            .expect("spawn inherited-pipe grandchild");

        wait_for_managed_listener_address(
            &mut grandchild,
            Path::new(&ready_path),
            Duration::from_secs(1),
        )
        .expect("inherited-pipe grandchild did not become ready");
        drop(grandchild);
    }

    #[test]
    fn delayed_ready_child_helper() {
        let Some(ready_path) = std::env::var_os("MERMAN_XTASK_SUPPORT_DELAYED_READY") else {
            return;
        };
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind delayed-ready listener");
        // Deliberately model the old non-atomic producer so the reader regression covers the
        // original empty-file race instead of relying only on the corrected publisher.
        fs::write(&ready_path, "").expect("publish empty readiness placeholder");
        std::thread::sleep(Duration::from_millis(150));
        fs::write(
            &ready_path,
            listener
                .local_addr()
                .expect("delayed-ready listener address")
                .to_string(),
        )
        .expect("write delayed readiness payload");
        std::thread::sleep(Duration::from_secs(30));
        drop(listener);
    }

    #[test]
    fn process_tree_grandchild_helper() {
        let Some(ready_path) = std::env::var_os("MERMAN_XTASK_SUPPORT_TREE_GRANDCHILD_READY")
        else {
            return;
        };
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind process-tree grandchild listener");
        let address = listener
            .local_addr()
            .expect("grandchild listener address")
            .to_string();
        if let Some(bound_ready_path) = std::env::var_os("MERMAN_XTASK_SUPPORT_TREE_BOUND_READY") {
            publish_ready_file_atomically(Path::new(&bound_ready_path), &address)
                .expect("publish process-tree bound address");
        }
        if std::env::var_os("MERMAN_XTASK_SUPPORT_TREE_SKIP_READY").is_none() {
            let payload = if std::env::var_os("MERMAN_XTASK_SUPPORT_TREE_INVALID_READY").is_some() {
                "not-a-listener-address".to_string()
            } else {
                address
            };
            publish_ready_file_atomically(Path::new(&ready_path), &payload)
                .expect("write process-tree ready file");
        }
        std::thread::sleep(Duration::from_secs(30));
        drop(listener);
    }

    #[test]
    fn process_tree_child_helper() {
        let Some(ready_path) = std::env::var_os("MERMAN_XTASK_SUPPORT_TREE_CHILD_READY") else {
            return;
        };
        let executable = std::env::current_exe().expect("current test executable");
        let grandchild_test = exact_test_name("process_tree_grandchild_helper");
        let mut grandchild = Command::new(executable)
            .args(["--exact", grandchild_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_TREE_GRANDCHILD_READY", ready_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn process-tree grandchild");
        std::thread::sleep(Duration::from_secs(30));
        let _ = grandchild.wait();
    }

    #[test]
    fn timeout_terminates_the_managed_process_tree() {
        let root = unique_test_root("upstream-svg-support-process-tree");
        fs::create_dir_all(&root).expect("create process-tree test root");
        let ready_path = root.join("grandchild-ready.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("process_tree_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_TREE_CHILD_READY", &ready_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child =
            spawn_timeout_managed_child(&mut command).expect("spawn process-tree child");

        let address =
            wait_for_managed_listener_address(&mut child, &ready_path, Duration::from_secs(5))
                .expect("wait for process-tree grandchild listener");

        let error = wait_with_timeout(&mut child, Duration::from_millis(100))
            .expect_err("managed process tree must time out");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            child
                .try_wait()
                .expect("query process-tree child")
                .is_some(),
            "timed-out child must be reaped"
        );

        wait_for_listener_release(address);
        fs::remove_file(&ready_path).expect("remove process-tree ready file");
        fs::remove_dir(&root).expect("remove process-tree test root");
    }

    #[test]
    fn readiness_wait_retries_an_empty_file_until_it_is_parseable() {
        let root = unique_test_root("upstream-svg-support-delayed-ready");
        fs::create_dir_all(&root).expect("create delayed-ready test root");
        let ready_path = root.join("listener-ready.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("delayed_ready_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_DELAYED_READY", &ready_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child =
            spawn_timeout_managed_child(&mut command).expect("spawn delayed-ready child");

        let address =
            wait_for_managed_listener_address(&mut child, &ready_path, Duration::from_secs(5))
                .expect("wait through the empty readiness window");

        assert!(
            child
                .try_wait()
                .expect("query delayed-ready child")
                .is_none(),
            "a transient empty readiness file must not terminate a healthy child"
        );
        terminate_child_process_tree(&mut child);
        assert!(child.try_wait().expect("query terminated child").is_some());
        wait_for_listener_release(address);
        fs::remove_file(&ready_path).expect("remove delayed-ready file");
        fs::remove_dir(&root).expect("remove delayed-ready test root");
    }

    #[test]
    fn readiness_read_failure_terminates_and_reaps_the_managed_child() {
        let root = unique_test_root("upstream-svg-support-readiness-read-failure");
        let ready_path = root.join("ready-is-a-directory");
        fs::create_dir_all(&ready_path).expect("create invalid readiness directory");
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("timeout_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_TIMEOUT_CHILD", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_timeout_managed_child(&mut command).expect("spawn readiness child");

        wait_for_managed_listener_address(&mut child, &ready_path, Duration::from_secs(5))
            .expect_err("a readiness read failure must be reported");

        assert!(
            child
                .try_wait()
                .expect("query read-failure child")
                .is_some(),
            "a readiness read failure must reap the managed child"
        );
        fs::remove_dir(&ready_path).expect("remove invalid readiness directory");
        fs::remove_dir(&root).expect("remove readiness read-failure root");
    }

    #[test]
    fn readiness_child_exit_is_reported_and_reaped() {
        let root = unique_test_root("upstream-svg-support-readiness-child-exit");
        fs::create_dir_all(&root).expect("create readiness child-exit root");
        let ready_path = root.join("never-created.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("timeout_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_timeout_managed_child(&mut command).expect("spawn exiting child");

        let error =
            wait_for_managed_listener_address(&mut child, &ready_path, Duration::from_secs(5))
                .expect_err("an exited child cannot publish readiness");

        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        assert!(child.try_wait().expect("query exited child").is_some());
        assert!(!ready_path.exists());
        fs::remove_dir(&root).expect("remove readiness child-exit root");
    }

    #[test]
    fn readiness_payload_after_deadline_is_rejected_and_reaped() {
        let root = unique_test_root("upstream-svg-support-late-readiness");
        fs::create_dir_all(&root).expect("create late-readiness root");
        let ready_path = root.join("listener-ready.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("delayed_ready_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_DELAYED_READY", &ready_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_timeout_managed_child(&mut command).expect("spawn late-ready child");

        let error =
            wait_for_managed_listener_address(&mut child, &ready_path, Duration::from_millis(50))
                .expect_err("readiness published after the hard deadline must be rejected");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(child.try_wait().expect("query late-ready child").is_some());
        if ready_path.exists() {
            fs::remove_file(&ready_path).expect("remove late readiness file");
        }
        fs::remove_dir(&root).expect("remove late-readiness root");
    }

    #[test]
    fn readiness_timeout_terminates_the_tree_and_releases_the_grandchild_port() {
        let root = unique_test_root("upstream-svg-support-readiness-timeout");
        fs::create_dir_all(&root).expect("create readiness-timeout test root");
        let ready_path = root.join("never-ready.txt");
        let bound_ready_path = root.join("listener-bound.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("process_tree_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_TREE_CHILD_READY", &ready_path)
            .env("MERMAN_XTASK_SUPPORT_TREE_BOUND_READY", &bound_ready_path)
            .env("MERMAN_XTASK_SUPPORT_TREE_SKIP_READY", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child =
            spawn_timeout_managed_child(&mut command).expect("spawn never-ready process tree");

        let observed_address = wait_for_managed_listener_address(
            &mut child,
            &bound_ready_path,
            Duration::from_secs(5),
        )
        .expect("wait for the never-ready grandchild to bind");

        let error =
            wait_for_managed_listener_address(&mut child, &ready_path, Duration::from_millis(250))
                .expect_err("the process tree must never publish readiness");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            child.try_wait().expect("query never-ready child").is_some(),
            "a readiness timeout must reap the managed parent"
        );
        wait_for_listener_release(observed_address);
        assert!(!ready_path.exists());
        fs::remove_file(&bound_ready_path).expect("remove bound readiness file");
        fs::remove_dir(&root).expect("remove readiness-timeout test root");
    }

    #[test]
    fn invalid_readiness_terminates_the_tree_and_releases_the_grandchild_port() {
        let root = unique_test_root("upstream-svg-support-invalid-readiness");
        fs::create_dir_all(&root).expect("create invalid-readiness test root");
        let ready_path = root.join("invalid-ready.txt");
        let bound_ready_path = root.join("listener-bound.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("process_tree_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_TREE_CHILD_READY", &ready_path)
            .env("MERMAN_XTASK_SUPPORT_TREE_BOUND_READY", &bound_ready_path)
            .env("MERMAN_XTASK_SUPPORT_TREE_INVALID_READY", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child =
            spawn_timeout_managed_child(&mut command).expect("spawn invalid-ready process tree");

        let observed_address = wait_for_managed_listener_address(
            &mut child,
            &bound_ready_path,
            Duration::from_secs(5),
        )
        .expect("wait for the invalid-ready grandchild to bind");

        let error =
            wait_for_managed_listener_address(&mut child, &ready_path, Duration::from_millis(250))
                .expect_err("the invalid readiness payload must never be accepted");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            error
                .to_string()
                .contains("last readiness payload was invalid")
        );
        assert!(
            child
                .try_wait()
                .expect("query invalid-ready child")
                .is_some(),
            "an invalid readiness payload must reap the managed parent"
        );
        wait_for_listener_release(observed_address);
        fs::remove_file(&ready_path).expect("remove invalid readiness file");
        fs::remove_file(&bound_ready_path).expect("remove bound readiness file");
        fs::remove_dir(&root).expect("remove invalid-readiness test root");
    }

    #[test]
    fn process_wait_enforces_a_hard_timeout() {
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("timeout_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_TIMEOUT_CHILD", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_timeout_managed_child(&mut command).expect("spawn timeout child");
        let started = Instant::now();

        let error = wait_with_timeout(&mut child, Duration::from_millis(100))
            .expect_err("sleeping child must time out");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(child.try_wait().expect("query terminated child").is_some());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn bounded_output_drains_stdout_and_stderr_without_backpressure() {
        let executable = std::env::current_exe().expect("current test executable");
        let child_test = exact_test_name("large_pipe_child_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", child_test.as_str(), "--nocapture"])
            .env("MERMAN_XTASK_SUPPORT_LARGE_PIPE_CHILD", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawn_timeout_managed_child(&mut command).expect("spawn large-pipe child");

        let output = wait_with_bounded_output(&mut child, Duration::from_secs(5), 1024 * 1024)
            .expect("large output should be drained concurrently");

        assert!(output.status.success());
        assert!(output.stdout.iter().filter(|byte| **byte == b'o').count() >= 512 * 1024);
        assert!(output.stderr.iter().filter(|byte| **byte == b'e').count() >= 512 * 1024);
    }

    #[test]
    fn bounded_output_deadline_includes_inherited_pipe_drain() {
        let root = unique_test_root("upstream-svg-support-inherited-pipe");
        fs::create_dir_all(&root).expect("create inherited-pipe test root");
        let ready_path = root.join("grandchild-ready.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let parent_test = exact_test_name("inherited_pipe_parent_helper");
        let mut command = Command::new(executable);
        command
            .args(["--exact", parent_test.as_str(), "--nocapture"])
            .env(
                "MERMAN_XTASK_SUPPORT_INHERITED_PIPE_PARENT_READY",
                &ready_path,
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child =
            spawn_timeout_managed_child(&mut command).expect("spawn inherited-pipe parent");
        let started = Instant::now();

        let error = wait_with_bounded_output(&mut child, Duration::from_secs(2), 1024 * 1024)
            .expect_err("an inherited writer must not outlive the shared deadline");

        assert!(
            error.to_string().contains("timed out while draining"),
            "{error}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "pipe drain exceeded its hard deadline: {:?}",
            started.elapsed()
        );
        assert!(
            child
                .try_wait()
                .expect("query inherited-pipe parent")
                .is_some(),
            "the exited parent must remain reaped"
        );

        let address: SocketAddr = fs::read_to_string(&ready_path)
            .expect("read inherited-pipe ready file")
            .parse()
            .expect("parse inherited-pipe grandchild address");
        wait_for_listener_release(address);
        fs::remove_file(&ready_path).expect("remove inherited-pipe ready file");
        fs::remove_dir(&root).expect("remove inherited-pipe test root");
    }

    #[test]
    fn bounded_reader_drains_to_eof_after_reaching_its_limit() {
        let mut cursor = std::io::Cursor::new(vec![b'x'; 4096]);

        let error = read_bounded_child_pipe(&mut cursor, "test", 1024)
            .expect_err("oversized output must be rejected");

        assert!(error.to_string().contains("exceeded 1024 bytes"));
        assert_eq!(cursor.position(), 4096);
    }
}
