//! Shared process and generated-file support for upstream SVG tooling.

use crate::XtaskError;
use sha2::{Digest, Sha256};
use std::cmp::Ordering as CmpOrdering;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output};
use std::sync::atomic::{AtomicU64, Ordering};
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
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    };

    const MAX_REFRESH_PASSES: usize = 4;

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
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
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

pub(crate) fn terminate_child_process_tree(child: &mut Child) {
    #[cfg(windows)]
    let process_tree = windows_process_tree::ProcessTree::capture(child.id());
    #[cfg(unix)]
    {
        let process_group = libc::pid_t::try_from(child.id()).ok();
        if let Some(process_group) = process_group.filter(|process_group| *process_group > 0) {
            // SAFETY: the positive PGID is converted without truncation from a child created by
            // spawn_timeout_managed_child with process_group(0). Negating it therefore targets
            // only that child's process group, never kill(0, ...) or kill(-1, ...).
            let _ = unsafe { libc::kill(-process_group, libc::SIGKILL) };
        }
    }
    let _ = child.kill();
    #[cfg(windows)]
    process_tree.terminate_descendants();
    let _ = child.wait();
}

pub(crate) fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
) -> std::io::Result<ExitStatus> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(err) => {
                terminate_child_process_tree(child);
                return Err(err);
            }
        }
        if start.elapsed() >= timeout {
            terminate_child_process_tree(child);
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "process timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
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

fn join_child_pipe_reader(
    reader: std::thread::JoinHandle<Result<Vec<u8>, XtaskError>>,
    description: &str,
) -> Result<Vec<u8>, XtaskError> {
    reader.join().map_err(|_| {
        XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render probe {description} reader panicked"
        ))
    })?
}

pub(crate) fn wait_with_bounded_output(
    child: &mut Child,
    timeout: Duration,
    max_bytes_per_pipe: u64,
) -> Result<Output, XtaskError> {
    let stdout = child.stdout.take().ok_or_else(|| {
        terminate_child_process_tree(child);
        XtaskError::UpstreamSvgFailed(
            "upstream SVG render probe stdout was not captured".to_string(),
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        terminate_child_process_tree(child);
        XtaskError::UpstreamSvgFailed(
            "upstream SVG render probe stderr was not captured".to_string(),
        )
    })?;

    let stdout_reader = std::thread::Builder::new()
        .name("upstream-svg-probe-stdout".to_string())
        .spawn(move || read_bounded_child_pipe(stdout, "stdout", max_bytes_per_pipe))
        .map_err(|err| {
            terminate_child_process_tree(child);
            XtaskError::UpstreamSvgFailed(format!(
                "failed to start upstream SVG render probe stdout reader: {err}"
            ))
        })?;
    let stderr_reader = match std::thread::Builder::new()
        .name("upstream-svg-probe-stderr".to_string())
        .spawn(move || read_bounded_child_pipe(stderr, "stderr", max_bytes_per_pipe))
    {
        Ok(reader) => reader,
        Err(err) => {
            terminate_child_process_tree(child);
            let _ = stdout_reader.join();
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "failed to start upstream SVG render probe stderr reader: {err}"
            )));
        }
    };

    let status = wait_with_timeout(child, timeout);
    let stdout = join_child_pipe_reader(stdout_reader, "stdout");
    let stderr = join_child_pipe_reader(stderr_reader, "stderr");

    let status = status.map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render environment probe failed: {err}"
        ))
    })?;
    Ok(Output {
        status,
        stdout: stdout?,
        stderr: stderr?,
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
    use std::net::{SocketAddr, TcpListener};
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
    fn process_tree_grandchild_helper() {
        let Some(ready_path) = std::env::var_os("MERMAN_XTASK_SUPPORT_TREE_GRANDCHILD_READY")
        else {
            return;
        };
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("bind process-tree grandchild listener");
        fs::write(
            ready_path,
            listener
                .local_addr()
                .expect("grandchild listener address")
                .to_string(),
        )
        .expect("write process-tree ready file");
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

        let ready_deadline = Instant::now() + Duration::from_secs(5);
        while !ready_path.is_file() && Instant::now() < ready_deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let address: SocketAddr = fs::read_to_string(&ready_path)
            .expect("read process-tree ready file")
            .parse()
            .expect("parse grandchild listener address");

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

        let release_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match TcpListener::bind(address) {
                Ok(listener) => {
                    drop(listener);
                    break;
                }
                Err(_) if Instant::now() < release_deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => panic!("grandchild listener remained alive after timeout: {err}"),
            }
        }
        fs::remove_file(&ready_path).expect("remove process-tree ready file");
        fs::remove_dir(&root).expect("remove process-tree test root");
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
    fn bounded_reader_drains_to_eof_after_reaching_its_limit() {
        let mut cursor = std::io::Cursor::new(vec![b'x'; 4096]);

        let error = read_bounded_child_pipe(&mut cursor, "test", 1024)
            .expect_err("oversized output must be rejected");

        assert!(error.to_string().contains("exceeded 1024 bytes"));
        assert_eq!(cursor.position(), 4096);
    }
}
