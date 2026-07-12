use crate::XtaskError;
#[cfg(test)]
use crate::cmd::read_bounded_child_pipe;
use crate::cmd::{
    ensure_content_addressed_js_script, ensure_upstream_svg_puppeteer_config,
    spawn_timeout_managed_child, upstream_svg_package_tree_sha256, wait_with_bounded_output,
    wait_with_timeout,
};
use crate::svgdom;
use crate::util::{extract_add_to_set_string_array, extract_defaults, extract_frozen_string_array};
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::collections::BTreeSet;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMPURIFY_BASELINE_VERSION: &str = "3.4.0";
const PINNED_MERMAID_PACKAGE_SHA256: &str =
    "9182344905d95e67ff6d5baf0f902a73bc77ee007aa6bcc5f7833ef133505a1b";
const PINNED_MERMAID_CLI_PACKAGE_SHA256: &str =
    "de9d9ac0cb0e2c55fa7cac7b3d4883bb76bf6d137eec290ed345101d8c0da632";

const UPSTREAM_SVG_DIAGRAMS: &[&str] = &[
    "er",
    "flowchart",
    "state",
    "class",
    "sequence",
    "info",
    "pie",
    "requirement",
    "sankey",
    "packet",
    "timeline",
    "journey",
    "kanban",
    "gitgraph",
    "gantt",
    "c4",
    "block",
    "radar",
    "quadrantchart",
    "treemap",
    "xychart",
    "mindmap",
    "treeView",
    "ishikawa",
    "eventmodeling",
    "architecture",
    "venn",
    "cynefin",
    "railroad",
    "railroadEbnf",
    "railroadAbnf",
    "railroadPeg",
];

static UPSTREAM_SVG_CHECK_RUN_COUNTER: AtomicU64 = AtomicU64::new(0);
static UPSTREAM_SVG_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn upstream_svg_supported_diagrams_message() -> String {
    format!("{}, all", UPSTREAM_SVG_DIAGRAMS.join(", "))
}

fn read_package_manifest(path: &Path) -> Result<JsonValue, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!(
            "failed to parse package metadata {}: {err}",
            path.display()
        ))
    })
}

fn required_package_manifest_string(
    manifest: &JsonValue,
    manifest_path: &Path,
    fields: &[&str],
) -> Result<String, XtaskError> {
    let value = fields
        .iter()
        .try_fold(manifest, |value, field| value.get(field))
        .and_then(JsonValue::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            XtaskError::UpstreamSvgFailed(format!(
                "package metadata {} must contain an exact string at {}",
                manifest_path.display(),
                fields.join(".")
            ))
        })?;
    Ok(value.to_string())
}

pub(crate) fn validate_mermaid_cli_install(tools_root: &Path) -> Result<PathBuf, XtaskError> {
    let tools_manifest_path = tools_root.join("package.json");
    let tools_manifest = read_package_manifest(&tools_manifest_path)?;
    let pinned_cli = required_package_manifest_string(
        &tools_manifest,
        &tools_manifest_path,
        &["devDependencies", "@mermaid-js/mermaid-cli"],
    )?;
    let pinned_mermaid = required_package_manifest_string(
        &tools_manifest,
        &tools_manifest_path,
        &["overrides", "mermaid"],
    )?;

    let mermaid_cli_root = tools_root.join("node_modules/@mermaid-js/mermaid-cli");
    let mermaid_cli_manifest_path = mermaid_cli_root.join("package.json");
    let installed_packages = [
        (
            "@mermaid-js/mermaid-cli",
            mermaid_cli_manifest_path.clone(),
            pinned_cli,
        ),
        (
            "mermaid",
            tools_root.join("node_modules/mermaid/package.json"),
            pinned_mermaid,
        ),
    ];

    for (package_name, installed_manifest_path, pinned_version) in installed_packages {
        let installed_manifest = read_package_manifest(&installed_manifest_path)?;
        let installed_version = required_package_manifest_string(
            &installed_manifest,
            &installed_manifest_path,
            &["version"],
        )?;
        if installed_version != pinned_version {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "installed {package_name} version {installed_version} does not match the pinned package metadata version {pinned_version} in {}; rerun with `--install` or run `npm ci` in {}",
                tools_manifest_path.display(),
                tools_root.display()
            )));
        }
    }

    let mermaid_cli_manifest = read_package_manifest(&mermaid_cli_manifest_path)?;
    let entry = required_package_manifest_string(
        &mermaid_cli_manifest,
        &mermaid_cli_manifest_path,
        &["bin", "mmdc"],
    )?;
    let entry = PathBuf::from(entry);
    if entry.is_absolute() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "installed Mermaid CLI bin.mmdc must be relative to its package root: {}",
            entry.display()
        )));
    }
    let canonical_package_root =
        fs::canonicalize(&mermaid_cli_root).map_err(|source| XtaskError::ReadFile {
            path: mermaid_cli_root.display().to_string(),
            source,
        })?;
    let entry_path = mermaid_cli_root.join(entry);
    let canonical_entry = fs::canonicalize(&entry_path).map_err(|source| XtaskError::ReadFile {
        path: entry_path.display().to_string(),
        source,
    })?;
    if !canonical_entry.starts_with(&canonical_package_root) || !canonical_entry.is_file() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "installed Mermaid CLI bin.mmdc must resolve to a file inside {}: {}",
            canonical_package_root.display(),
            canonical_entry.display()
        )));
    }

    // Keep the canonical paths only for containment validation. Node 24 cannot execute the
    // `\\?\C:\...` verbatim paths returned by fs::canonicalize on Windows, while the original
    // absolute drive or UNC spelling remains executable.
    Ok(entry_path)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamSvgRuntimePackageRoots {
    mermaid: PathBuf,
    mermaid_cli: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpstreamSvgRenderProbe {
    render_environment: crate::cmd::UpstreamSvgRenderEnvironment,
    browser_executable: PathBuf,
    runtime_package_roots: UpstreamSvgRuntimePackageRoots,
}

impl UpstreamSvgRenderProbe {
    fn verified_render_environment(
        &self,
    ) -> Result<crate::cmd::UpstreamSvgRenderEnvironment, XtaskError> {
        self.render_environment.validate()?;
        for (package_name, root, expected) in [
            (
                "mermaid",
                &self.runtime_package_roots.mermaid,
                self.render_environment
                    .mermaid_runtime
                    .mermaid_package_sha256
                    .as_str(),
            ),
            (
                "@mermaid-js/mermaid-cli",
                &self.runtime_package_roots.mermaid_cli,
                self.render_environment
                    .mermaid_runtime
                    .mermaid_cli_package_sha256
                    .as_str(),
            ),
        ] {
            validate_upstream_svg_runtime_package_root(root, package_name)?;
            let actual = upstream_svg_package_tree_sha256(root)?;
            if actual != expected {
                return Err(XtaskError::UpstreamSvgFailed(format!(
                    "upstream SVG runtime package {package_name} changed after the render-environment probe: expected={expected}, actual={actual}"
                )));
            }
        }
        Ok(self.render_environment.clone())
    }
}

fn validate_upstream_svg_runtime_package_root(
    root: &Path,
    package_name: &str,
) -> Result<(), XtaskError> {
    if !root.is_absolute() || !root.is_dir() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render probe returned an invalid {package_name} package root: {}",
            root.display()
        )));
    }
    let manifest_path = root.join("package.json");
    let manifest = read_package_manifest(&manifest_path)?;
    let actual_name = required_package_manifest_string(&manifest, &manifest_path, &["name"])?;
    if actual_name != package_name {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG runtime package root {} contains {actual_name:?}, expected {package_name:?}",
            root.display()
        )));
    }
    Ok(())
}

fn installed_mermaid_version(tools_root: &Path) -> Result<String, XtaskError> {
    let manifest_path = tools_root.join("node_modules/mermaid/package.json");
    let manifest = read_package_manifest(&manifest_path)?;
    required_package_manifest_string(&manifest, &manifest_path, &["version"])
}

fn validate_upstream_svg_render_probe(
    probe: UpstreamSvgRenderProbe,
    installed_mermaid_version: &str,
) -> Result<UpstreamSvgRenderProbe, XtaskError> {
    probe.render_environment.validate()?;
    if !probe.browser_executable.is_absolute() || !probe.browser_executable.is_file() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render probe returned an invalid browser executable: {}",
            probe.browser_executable.display()
        )));
    }
    validate_upstream_svg_runtime_package_root(&probe.runtime_package_roots.mermaid, "mermaid")?;
    validate_upstream_svg_runtime_package_root(
        &probe.runtime_package_roots.mermaid_cli,
        "@mermaid-js/mermaid-cli",
    )?;

    let runtimes = &probe.render_environment.mermaid_runtime;
    if runtimes.esm_version != installed_mermaid_version
        || runtimes.iife_version != installed_mermaid_version
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render probe loaded unexpected Mermaid runtimes: ESM={}, IIFE={}, installed={installed_mermaid_version}",
            runtimes.esm_version, runtimes.iife_version
        )));
    }
    if runtimes.mermaid_package_sha256 != PINNED_MERMAID_PACKAGE_SHA256
        || runtimes.mermaid_cli_package_sha256 != PINNED_MERMAID_CLI_PACKAGE_SHA256
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "installed Mermaid runtime package content does not match the pinned 11.16.0 artifacts: mermaid={}, mermaid-cli={}",
            runtimes.mermaid_package_sha256, runtimes.mermaid_cli_package_sha256
        )));
    }

    Ok(probe)
}

fn probe_upstream_svg_render_environment(
    tools_root: &Path,
) -> Result<UpstreamSvgRenderProbe, XtaskError> {
    let script_path = ensure_upstream_svg_render_environment_probe_script()?;
    let mut command = Command::new("node");
    command
        .arg(&script_path)
        .current_dir(tools_root)
        .env("PUPPETEER_BROWSER", "chrome")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_timeout_managed_child(&mut command).map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!(
            "failed to run upstream SVG render environment probe {}: {err}",
            script_path.display()
        ))
    })?;
    const MAX_PROBE_OUTPUT_BYTES: u64 = 1024 * 1024;
    let output =
        wait_with_bounded_output(&mut child, Duration::from_secs(60), MAX_PROBE_OUTPUT_BYTES)
            .map_err(|err| {
                XtaskError::UpstreamSvgFailed(format!(
                    "upstream SVG render environment probe {} failed: {err}",
                    script_path.display()
                ))
            })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG render environment probe failed (exit={}): {stderr}",
            output.status.code().unwrap_or(-1)
        )));
    }

    let probe: UpstreamSvgRenderProbe = serde_json::from_slice(&output.stdout).map_err(|err| {
        XtaskError::UpstreamSvgFailed(format!(
            "failed to decode upstream SVG render environment probe output: {err}"
        ))
    })?;
    validate_upstream_svg_render_probe(probe, &installed_mermaid_version(tools_root)?)
}

fn create_upstream_svg_check_output_root(target_root: &Path) -> Result<PathBuf, XtaskError> {
    let check_root = target_root.join("upstream-svgs-check");
    fs::create_dir_all(&check_root).map_err(|source| XtaskError::WriteFile {
        path: check_root.display().to_string(),
        source,
    })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..128 {
        let sequence = UPSTREAM_SVG_CHECK_RUN_COUNTER.fetch_add(1, Ordering::Relaxed);
        let output_root =
            check_root.join(format!("run-{}-{timestamp}-{sequence}", std::process::id()));
        match fs::create_dir(&output_root) {
            Ok(()) => return Ok(output_root),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(XtaskError::WriteFile {
                    path: output_root.display().to_string(),
                    source,
                });
            }
        }
    }

    Err(XtaskError::UpstreamSvgFailed(format!(
        "failed to allocate a unique upstream SVG check output under {}",
        check_root.display()
    )))
}

fn unique_upstream_svg_temp_path(staging_dir: &Path, out_path: &Path) -> PathBuf {
    let sequence = UPSTREAM_SVG_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = out_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upstream.svg");
    staging_dir.join(format!(
        ".{file_name}.{}.{timestamp}.{sequence}.tmp.svg",
        std::process::id(),
    ))
}

fn unique_upstream_svg_failure_report_path(staging_dir: &Path) -> PathBuf {
    unique_upstream_svg_temp_path(staging_dir, Path::new("_failures.txt")).with_extension("txt")
}

fn cleanup_upstream_svg_temp(temp_path: &Path) -> Result<(), String> {
    match fs::remove_file(temp_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!(
            "failed to clean temporary upstream SVG {}: {err}",
            temp_path.display()
        )),
    }
}

fn upstream_svg_failure_with_cleanup(temp_path: &Path, message: String) -> String {
    match cleanup_upstream_svg_temp(temp_path) {
        Ok(()) => message,
        Err(cleanup) => format!("{message}; {cleanup}"),
    }
}

#[derive(Debug)]
struct PendingUpstreamSvg {
    temp_path: PathBuf,
    out_path: PathBuf,
}

#[derive(Debug)]
struct StagedUpstreamSvg {
    out_path: PathBuf,
    backup_path: Option<PathBuf>,
}

fn validate_upstream_svg_temp(temp_path: &Path) -> Result<(), String> {
    let bytes = fs::read(temp_path).map_err(|err| {
        format!(
            "upstream renderer did not produce temporary SVG {}: {err}",
            temp_path.display()
        )
    })?;
    if bytes.is_empty() {
        return Err(format!(
            "upstream renderer produced an empty temporary SVG {}",
            temp_path.display()
        ));
    }
    if !bytes.windows(b"<svg".len()).any(|window| window == b"<svg") {
        return Err(format!(
            "upstream renderer output is not an SVG document: {}",
            temp_path.display()
        ));
    }
    Ok(())
}

fn unique_upstream_svg_backup_path(out_path: &Path) -> PathBuf {
    let sequence = UPSTREAM_SVG_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = out_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upstream.svg");
    out_path.with_file_name(format!(
        ".{file_name}.{}.{sequence}.backup",
        std::process::id()
    ))
}

fn cleanup_pending_upstream_svg_temps(pending: &[PendingUpstreamSvg]) -> Vec<String> {
    pending
        .iter()
        .filter_map(|entry| cleanup_upstream_svg_temp(&entry.temp_path).err())
        .collect()
}

fn rollback_upstream_svg_batch(
    staged: &[StagedUpstreamSvg],
    installed_replacements: &[PathBuf],
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut retained_targets = BTreeSet::new();
    for out_path in installed_replacements.iter().rev() {
        match fs::remove_file(out_path) {
            Ok(()) => true,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
            Err(err) => {
                errors.push(format!(
                    "failed to remove promoted upstream SVG {} during rollback: {err}",
                    out_path.display()
                ));
                retained_targets.insert(out_path.clone());
                false
            }
        };
    }
    for entry in staged.iter().rev() {
        let Some(backup_path) = &entry.backup_path else {
            continue;
        };
        if retained_targets.contains(&entry.out_path) {
            continue;
        }
        if let Err(err) = fs::rename(backup_path, &entry.out_path) {
            errors.push(format!(
                "failed to restore upstream SVG {} from {}: {err}",
                entry.out_path.display(),
                backup_path.display()
            ));
        }
    }
    errors
}

fn with_batch_cleanup_errors(mut message: String, cleanup_errors: Vec<String>) -> String {
    if !cleanup_errors.is_empty() {
        message.push_str("; ");
        message.push_str(&cleanup_errors.join("; "));
    }
    message
}

fn promote_upstream_svg_batch<F>(
    pending: &[PendingUpstreamSvg],
    deletions: &[PathBuf],
    commit_metadata: F,
) -> Result<(), String>
where
    F: FnOnce() -> Result<(), String>,
{
    for entry in pending {
        if let Err(err) = validate_upstream_svg_temp(&entry.temp_path) {
            return Err(with_batch_cleanup_errors(
                err,
                cleanup_pending_upstream_svg_temps(pending),
            ));
        }
    }

    let mut targets = BTreeSet::new();
    for out_path in pending.iter().map(|entry| &entry.out_path).chain(deletions) {
        if !targets.insert(out_path.clone()) {
            return Err(with_batch_cleanup_errors(
                format!(
                    "duplicate upstream SVG transaction target {}",
                    out_path.display()
                ),
                cleanup_pending_upstream_svg_temps(pending),
            ));
        }
        if out_path.exists() && !out_path.is_file() {
            return Err(with_batch_cleanup_errors(
                format!(
                    "upstream SVG transaction target is not a file: {}",
                    out_path.display()
                ),
                cleanup_pending_upstream_svg_temps(pending),
            ));
        }
    }

    let mut staged = Vec::with_capacity(targets.len());
    for out_path in targets {
        let backup_path = if out_path.is_file() {
            let backup_path = unique_upstream_svg_backup_path(&out_path);
            if let Err(err) = fs::rename(&out_path, &backup_path) {
                let mut cleanup_errors = rollback_upstream_svg_batch(&staged, &[]);
                cleanup_errors.extend(cleanup_pending_upstream_svg_temps(pending));
                return Err(with_batch_cleanup_errors(
                    format!(
                        "failed to stage existing upstream SVG {}: {err}",
                        out_path.display()
                    ),
                    cleanup_errors,
                ));
            }
            Some(backup_path)
        } else {
            None
        };
        staged.push(StagedUpstreamSvg {
            out_path,
            backup_path,
        });
    }

    let mut installed_replacements = Vec::with_capacity(pending.len());
    for entry in pending {
        if let Err(err) = fs::rename(&entry.temp_path, &entry.out_path) {
            let mut cleanup_errors = rollback_upstream_svg_batch(&staged, &installed_replacements);
            cleanup_errors.extend(cleanup_pending_upstream_svg_temps(pending));
            return Err(with_batch_cleanup_errors(
                format!(
                    "failed to promote temporary upstream SVG {} to {}: {err}",
                    entry.temp_path.display(),
                    entry.out_path.display()
                ),
                cleanup_errors,
            ));
        }
        installed_replacements.push(entry.out_path.clone());
    }

    if let Err(err) = commit_metadata() {
        let mut cleanup_errors = rollback_upstream_svg_batch(&staged, &installed_replacements);
        cleanup_errors.extend(cleanup_pending_upstream_svg_temps(pending));
        return Err(with_batch_cleanup_errors(err, cleanup_errors));
    }

    for backup_path in staged.iter().filter_map(|entry| entry.backup_path.as_ref()) {
        if let Err(err) = fs::remove_file(backup_path) {
            eprintln!(
                "warning: failed to remove committed upstream SVG backup {}: {err}",
                backup_path.display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
fn validate_and_promote_upstream_svg_temp(temp_path: &Path, out_path: &Path) -> Result<(), String> {
    promote_upstream_svg_batch(
        &[PendingUpstreamSvg {
            temp_path: temp_path.to_path_buf(),
            out_path: out_path.to_path_buf(),
        }],
        &[],
        || Ok(()),
    )
}

fn parse_upstream_svg_jobs(raw: Option<&str>) -> Result<NonZeroUsize, XtaskError> {
    let Some(raw) = raw else {
        return Err(XtaskError::Usage);
    };
    raw.trim()
        .parse::<usize>()
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| {
            XtaskError::UpstreamSvgFailed(
                "`--jobs` must be an integer greater than or equal to 1".to_string(),
            )
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GenUpstreamSvgsOptions {
    diagram: String,
    out_root: Option<PathBuf>,
    filter: Option<String>,
    install: bool,
    fixtures_root: Option<PathBuf>,
    jobs: NonZeroUsize,
    fresh_output: bool,
}

impl Default for GenUpstreamSvgsOptions {
    fn default() -> Self {
        Self {
            diagram: "er".to_string(),
            out_root: None,
            filter: None,
            install: false,
            fixtures_root: None,
            jobs: NonZeroUsize::MIN,
            fresh_output: false,
        }
    }
}

fn required_gen_upstream_svg_option_value<'a>(
    args: &'a [String],
    index: &mut usize,
) -> Result<&'a str, XtaskError> {
    *index += 1;
    let value = args
        .get(*index)
        .map(String::as_str)
        .ok_or(XtaskError::Usage)?;
    if value.trim().is_empty() || value.starts_with('-') {
        return Err(XtaskError::Usage);
    }
    Ok(value)
}

fn parse_gen_upstream_svgs_options(args: &[String]) -> Result<GenUpstreamSvgsOptions, XtaskError> {
    let mut options = GenUpstreamSvgsOptions::default();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--diagram" => {
                options.diagram = required_gen_upstream_svg_option_value(args, &mut index)?
                    .trim()
                    .to_string();
            }
            "--out" => {
                options.out_root = Some(PathBuf::from(
                    required_gen_upstream_svg_option_value(args, &mut index)?.trim(),
                ));
            }
            "--filter" => {
                options.filter =
                    Some(required_gen_upstream_svg_option_value(args, &mut index)?.to_string());
            }
            "--fixtures-root" => {
                options.fixtures_root = Some(PathBuf::from(
                    required_gen_upstream_svg_option_value(args, &mut index)?.trim(),
                ));
            }
            "--jobs" => {
                let raw = required_gen_upstream_svg_option_value(args, &mut index)?;
                options.jobs = parse_upstream_svg_jobs(Some(raw))?;
            }
            "--fresh-output" => options.fresh_output = true,
            "--install" => options.install = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }
    if options.out_root.is_none() && (options.fixtures_root.is_some() || options.fresh_output) {
        return Err(XtaskError::Usage);
    }
    Ok(options)
}

fn absolutize_workspace_path(workspace_root: &Path, path: PathBuf) -> Result<PathBuf, XtaskError> {
    #[cfg(windows)]
    if !path.is_absolute()
        && (path.has_root()
            || matches!(
                path.components().next(),
                Some(std::path::Component::Prefix(_))
            ))
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG path uses an unsupported non-absolute Windows root or drive prefix: {}",
            path.display()
        )));
    }
    let resolved = if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    };
    if !resolved.is_absolute() {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "upstream SVG path did not resolve to an absolute workspace path: {}",
            resolved.display()
        )));
    }
    Ok(resolved)
}

fn upstream_svg_filter_matches(fixtures_dir: &Path, filter: &str) -> Vec<PathBuf> {
    crate::cmd::list_mmd_fixtures_in_dir(fixtures_dir, Some(filter), false)
}

fn validate_upstream_svg_filter_selection(
    fixtures_dir: &Path,
    filter: &str,
    expected: &[PathBuf],
) -> Result<(), XtaskError> {
    let actual = upstream_svg_filter_matches(fixtures_dir, filter);
    if actual == expected {
        return Ok(());
    }
    Err(XtaskError::UpstreamSvgFailed(format!(
        "upstream SVG fixture selection for filter {filter:?} changed while preparing {}; rerun generation",
        fixtures_dir.display()
    )))
}

fn select_upstream_svg_diagrams(
    diagram: &str,
    fixtures_root: &Path,
    filter: Option<&str>,
) -> Result<Vec<&'static str>, XtaskError> {
    let candidates = if diagram == "all" {
        UPSTREAM_SVG_DIAGRAMS.to_vec()
    } else {
        let target = UPSTREAM_SVG_DIAGRAMS
            .iter()
            .copied()
            .find(|candidate| *candidate == diagram)
            .ok_or_else(|| {
                XtaskError::UpstreamSvgFailed(format!(
                    "unsupported diagram for upstream svg export: {diagram} (supported: {})",
                    upstream_svg_supported_diagrams_message()
                ))
            })?;
        vec![target]
    };

    let Some(filter) = filter else {
        return Ok(candidates);
    };
    let selected = candidates
        .into_iter()
        .filter(|candidate| {
            !upstream_svg_filter_matches(&fixtures_root.join(*candidate), filter).is_empty()
        })
        .collect::<Vec<_>>();
    if !selected.is_empty() {
        return Ok(selected);
    }

    let location = if diagram == "all" {
        fixtures_root.to_path_buf()
    } else {
        fixtures_root.join(diagram)
    };
    Err(XtaskError::UpstreamSvgFailed(format!(
        "no .mmd fixtures matched filter {filter:?} under {}",
        location.display()
    )))
}

fn ensure_fresh_upstream_svg_output_is_empty(
    out_dir: &Path,
    fresh_output: bool,
) -> Result<(), XtaskError> {
    if !fresh_output {
        return Ok(());
    }
    let mut entries = fs::read_dir(out_dir).map_err(|source| XtaskError::ReadFile {
        path: out_dir.display().to_string(),
        source,
    })?;
    if entries
        .next()
        .transpose()
        .map_err(|source| XtaskError::ReadFile {
            path: out_dir.display().to_string(),
            source,
        })?
        .is_some()
    {
        return Err(XtaskError::UpstreamSvgFailed(format!(
            "refusing fresh upstream SVG generation into non-empty directory {}",
            out_dir.display()
        )));
    }
    Ok(())
}

#[derive(Debug)]
enum UpstreamSvgFamilyLockGuard<'a> {
    Borrowed(&'a crate::cmd::UpstreamSvgFamilyLock),
    Owned(crate::cmd::UpstreamSvgFamilyLock),
}

impl UpstreamSvgFamilyLockGuard<'_> {
    fn validate_target(&self, out_dir: &Path) -> Result<(), XtaskError> {
        match self {
            Self::Borrowed(lock) => lock.validate_target(out_dir),
            Self::Owned(lock) => lock.validate_target(out_dir),
        }
    }
}

fn use_or_acquire_upstream_svg_family_lock<'a>(
    out_dir: &Path,
    external_lock: Option<&'a crate::cmd::UpstreamSvgFamilyLock>,
) -> Result<UpstreamSvgFamilyLockGuard<'a>, XtaskError> {
    let guard = match external_lock {
        Some(lock) => UpstreamSvgFamilyLockGuard::Borrowed(lock),
        None => UpstreamSvgFamilyLockGuard::Owned(crate::cmd::acquire_upstream_svg_family_lock(
            out_dir,
        )?),
    };
    guard.validate_target(out_dir)?;
    Ok(guard)
}

fn validate_external_upstream_svg_family_lock(
    requested_diagram: &str,
    selected_diagrams: &[&str],
    out_root: &Path,
    family_lock: &crate::cmd::UpstreamSvgFamilyLock,
) -> Result<(), XtaskError> {
    if requested_diagram == "all" || selected_diagrams.len() != 1 {
        return Err(XtaskError::UpstreamSvgFailed(
            "generation under an existing upstream SVG family lock requires one explicit diagram"
                .to_string(),
        ));
    }
    family_lock.validate_target(&out_root.join(selected_diagrams[0]))
}

fn map_bounded_in_order<T, R, F>(items: &[T], jobs: NonZeroUsize, operation: F) -> Vec<R>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> R + Sync,
{
    let worker_count = jobs.get().min(items.len());
    if worker_count <= 1 {
        return items.iter().map(operation).collect();
    }

    let next_index = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let sender = sender.clone();
            let operation = &operation;
            let next_index = &next_index;
            scope.spawn(move || {
                loop {
                    let index = next_index.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(index) else {
                        break;
                    };
                    if sender.send((index, operation(item))).is_err() {
                        break;
                    }
                }
            });
        }
    });
    drop(sender);

    let mut indexed_results: Vec<_> = receiver.into_iter().collect();
    indexed_results.sort_unstable_by_key(|(index, _)| *index);
    debug_assert_eq!(indexed_results.len(), items.len());
    indexed_results
        .into_iter()
        .map(|(_, result)| result)
        .collect()
}

type PartitionedUpstreamSvgFixtures = (Vec<PathBuf>, Vec<(PathBuf, String)>);

fn partition_upstream_svg_fixtures(
    diagram: &str,
    fixture_files: impl IntoIterator<Item = PathBuf>,
) -> Result<PartitionedUpstreamSvgFixtures, XtaskError> {
    let mut renderable = Vec::new();
    let mut excluded = Vec::new();
    for path in fixture_files {
        if let Some(reason) = crate::cmd::upstream_svg_fixture_exclusion_reason(diagram, &path)? {
            excluded.push((path, reason));
        } else {
            renderable.push(path);
        }
    }
    renderable.sort();
    excluded.sort_by(|left, right| left.0.cmp(&right.0));
    Ok((renderable, excluded))
}

pub(crate) fn gen_upstream_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    gen_upstream_svgs_impl(args, None, None)
}

pub(crate) fn gen_upstream_svgs_with_transaction_locks(
    args: Vec<String>,
    family_lock: &crate::cmd::UpstreamSvgFamilyLock,
    toolchain_lock: &crate::cmd::UpstreamSvgToolchainLock,
) -> Result<(), XtaskError> {
    gen_upstream_svgs_impl(args, Some(family_lock), Some(toolchain_lock))
}

fn gen_upstream_svgs_impl(
    args: Vec<String>,
    external_family_lock: Option<&crate::cmd::UpstreamSvgFamilyLock>,
    external_toolchain_lock: Option<&crate::cmd::UpstreamSvgToolchainLock>,
) -> Result<(), XtaskError> {
    let GenUpstreamSvgsOptions {
        diagram,
        out_root: requested_out_root,
        filter: requested_filter,
        install,
        fixtures_root: requested_fixtures_root,
        jobs,
        fresh_output,
    } = parse_gen_upstream_svgs_options(&args)?;
    let workspace_root = crate::cmd::workspace_root();
    let fixtures_root = requested_fixtures_root
        .map(|path| absolutize_workspace_path(&workspace_root, path))
        .transpose()?
        .unwrap_or_else(crate::cmd::fixtures_root);
    let out_root = requested_out_root
        .map(|path| absolutize_workspace_path(&workspace_root, path))
        .transpose()?
        .unwrap_or_else(|| crate::cmd::fixtures_root().join("upstream-svgs"));
    let filter = requested_filter.as_deref();
    let selected_diagrams = select_upstream_svg_diagrams(&diagram, &fixtures_root, filter)?;
    if let Some(family_lock) = external_family_lock {
        validate_external_upstream_svg_family_lock(
            &diagram,
            &selected_diagrams,
            &out_root,
            family_lock,
        )?;
    }
    if diagram == "all"
        && let Some(filter) = filter
    {
        println!(
            "upstream SVG filter {filter:?} matched {} family/families for output {}: {}",
            selected_diagrams.len(),
            out_root.display(),
            selected_diagrams.join(", ")
        );
    }

    let tools_root = crate::cmd::mermaid_cli_root();
    let _owned_toolchain_lock = match external_toolchain_lock {
        Some(toolchain_lock) => {
            toolchain_lock.validate_target(&tools_root)?;
            None
        }
        None => Some(crate::cmd::acquire_upstream_svg_toolchain_lock(
            &tools_root,
        )?),
    };
    let node_modules = tools_root.join("node_modules");
    if install || !node_modules.exists() {
        let npm_cmd = if tools_root.join("package-lock.json").is_file() {
            "ci"
        } else {
            "install"
        };
        let mut cmd = if cfg!(windows) {
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("/c").arg("npm").arg(npm_cmd);
            cmd
        } else {
            let mut cmd = Command::new("npm");
            cmd.arg(npm_cmd);
            cmd
        };
        let status = cmd.current_dir(&tools_root).status().map_err(|err| {
            XtaskError::UpstreamSvgFailed(format!(
                "failed to run `npm {npm_cmd}` in {}: {err}",
                tools_root.display()
            ))
        })?;
        if !status.success() {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "npm {npm_cmd} failed in {}",
                tools_root.display()
            )));
        }
    }

    let mmdc = validate_mermaid_cli_install(&tools_root)?;
    let puppeteer_config = ensure_upstream_svg_puppeteer_config()?;
    let render_probe = probe_upstream_svg_render_environment(&tools_root)?;
    println!(
        "upstream SVG render environment: {}/{} (revision {}), Puppeteer {}, font probe {}",
        render_probe.render_environment.browser.product,
        render_probe.render_environment.browser.version,
        render_probe.render_environment.browser.revision,
        render_probe.render_environment.puppeteer.version,
        render_probe.render_environment.font_probe.sha256
    );

    struct UpstreamSvgGenerationContext<'a> {
        workspace_root: &'a Path,
        fixtures_root: &'a Path,
        out_root: &'a Path,
        mmdc: &'a Path,
        puppeteer_config: &'a Path,
        render_probe: &'a UpstreamSvgRenderProbe,
        jobs: NonZeroUsize,
        fresh_output: bool,
        external_family_lock: Option<&'a crate::cmd::UpstreamSvgFamilyLock>,
    }

    fn run_one(
        context: &UpstreamSvgGenerationContext<'_>,
        diagram: &str,
        filter: Option<&str>,
    ) -> Result<(), XtaskError> {
        let workspace_root = context.workspace_root;
        let fixtures_root = context.fixtures_root;
        let out_root = context.out_root;
        let mmdc = context.mmdc;
        let puppeteer_config = context.puppeteer_config;
        let render_probe = context.render_probe;
        let jobs = context.jobs;
        let fresh_output = context.fresh_output;
        let external_family_lock = context.external_family_lock;
        let fixtures_dir = fixtures_root.join(diagram);
        let out_dir = out_root.join(diagram);
        let requested_filter_matches = filter
            .map(|requested_filter| upstream_svg_filter_matches(&fixtures_dir, requested_filter));
        let requested_filter_match_count = requested_filter_matches.as_ref().map(Vec::len);
        if requested_filter_match_count == Some(0) {
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched filter {:?} under {}",
                filter.unwrap_or_default(),
                fixtures_dir.display()
            )));
        }
        let node_cwd = crate::cmd::mermaid_cli_root();
        let use_seeded_renderer = diagram == "architecture" || diagram == "gitgraph";
        let seeded_script = if use_seeded_renderer {
            Some(ensure_seeded_upstream_svg_renderer_script()?)
        } else {
            None
        };
        let per_chart_timeout = Duration::from_secs(60);

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;
        ensure_fresh_upstream_svg_output_is_empty(&out_dir, fresh_output)?;
        let requested_full_generation = filter.is_none();
        let initial_scope = match external_family_lock {
            Some(family_lock) => {
                family_lock.validate_target(&out_dir)?;
                crate::cmd::preflight_upstream_svg_provenance_write_under_family_lock(
                    &out_dir,
                    requested_full_generation,
                    fresh_output,
                    &render_probe.render_environment,
                )?
            }
            None => crate::cmd::preflight_upstream_svg_provenance_write(
                &out_dir,
                requested_full_generation,
                fresh_output,
                &render_probe.render_environment,
            )?,
        };
        let effective_filter = match initial_scope {
            crate::cmd::UpstreamSvgProvenanceWriteScope::Requested => filter,
            crate::cmd::UpstreamSvgProvenanceWriteScope::CompleteGenerationRequired => {
                let Some((requested_filter, match_count)) =
                    filter.zip(requested_filter_match_count)
                else {
                    return Err(XtaskError::UpstreamSvgFailed(format!(
                        "upstream SVG provenance requested an invalid complete-generation upgrade for {diagram}"
                    )));
                };
                println!(
                    "upstream SVG provenance for {diagram} is adopted-existing; filter {:?} matched {} fixture(s), upgrading to a complete family generation in {}",
                    requested_filter,
                    match_count,
                    out_dir.display()
                );
                None
            }
        };
        let full_generation = effective_filter.is_none();
        let staging_parent = out_root.join(".xtask-upstream-svg-staging");
        let mut fixture_snapshots = crate::cmd::capture_upstream_svg_fixture_selection(
            &staging_parent,
            diagram,
            &fixtures_dir,
            effective_filter,
        )?;
        if let Some((requested_filter, expected)) = filter.zip(requested_filter_matches.as_deref())
        {
            validate_upstream_svg_filter_selection(&fixtures_dir, requested_filter, expected)?;
        }
        let mmd_files = fixture_snapshots.renderable();
        let excluded_fixtures = fixture_snapshots.excluded();
        let excluded_count = excluded_fixtures.len();
        let excluded_svg_paths = excluded_fixtures
            .iter()
            .map(|exclusion| out_dir.join(format!("{}.svg", exclusion.fixture().stem())))
            .collect::<Vec<_>>();

        if mmd_files.is_empty() {
            if !excluded_fixtures.is_empty() {
                let family_lock =
                    use_or_acquire_upstream_svg_family_lock(&out_dir, external_family_lock)?;
                ensure_fresh_upstream_svg_output_is_empty(&out_dir, fresh_output)?;
                let final_scope =
                    crate::cmd::preflight_upstream_svg_provenance_write_under_family_lock(
                        &out_dir,
                        full_generation,
                        fresh_output,
                        &render_probe.render_environment,
                    )?;
                if final_scope != crate::cmd::UpstreamSvgProvenanceWriteScope::Requested {
                    return Err(XtaskError::UpstreamSvgFailed(format!(
                        "upstream SVG provenance changed while preparing {diagram}; rerun generation"
                    )));
                }
                fixture_snapshots.validate_live_selection_and_hashes()?;
                let verified_environment = render_probe.verified_render_environment()?;
                let commit = || {
                    fixture_snapshots
                        .validate_live_selection_and_hashes()
                        .map_err(|err| err.to_string())?;
                    crate::cmd::write_upstream_svg_provenance(
                        crate::cmd::UpstreamSvgProvenanceWriteRequest {
                            diagram,
                            fixtures_dir: &fixtures_dir,
                            out_dir: &out_dir,
                            generated_fixtures: fixture_snapshots.renderable(),
                            excluded_fixtures: fixture_snapshots.excluded(),
                            full_generation,
                            fresh_output,
                            render_environment: verified_environment,
                        },
                        || fixture_snapshots.validate_live_selection_and_hashes(),
                    )
                    .map_err(|err| err.to_string())
                };
                let promotion = promote_upstream_svg_batch(&[], &excluded_svg_paths, commit);
                drop(family_lock);
                let snapshot_cleanup = fixture_snapshots.cleanup();
                if let Err(message) = promotion {
                    return Err(XtaskError::UpstreamSvgFailed(match snapshot_cleanup {
                        Ok(()) => message,
                        Err(cleanup) => format!("{message}; {cleanup}"),
                    }));
                }
                if let Err(cleanup) = snapshot_cleanup {
                    eprintln!("warning: {cleanup}");
                }
                println!(
                    "skipped {} upstream svg fixture(s) for {diagram}: known upstream render gap",
                    excluded_count
                );
                return Ok(());
            }
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        let staging_dir = staging_parent.join(diagram);
        fs::create_dir_all(&staging_dir).map_err(|source| XtaskError::WriteFile {
            path: staging_dir.display().to_string(),
            source,
        })?;
        let failures_path = unique_upstream_svg_failure_report_path(&staging_dir);

        let render_results = map_bounded_in_order(mmd_files, jobs, |fixture| {
            let stem = fixture.stem();
            let mmd_path = fixture.live_path();
            let snapshot_path = fixture.snapshot_path();
            let out_path = out_dir.join(format!("{stem}.svg"));
            let temp_out_path = unique_upstream_svg_temp_path(&staging_dir, &out_path);
            let svg_id = crate::cmd::upstream_svg_id(stem);

            let status = if use_seeded_renderer {
                use std::io::Write;
                use std::process::Stdio;

                // Architecture layout relies on cytoscape-fcose, which uses `Math.random()` for
                // spectral initialization. To keep upstream baselines reproducible, we render via
                // a small puppeteer wrapper that seeds `Math.random()` deterministically.
                let pinned_config = node_cwd.join("mermaid-config.json");
                let seed: u64 = 1;
                let output_abs = if temp_out_path.is_absolute() {
                    temp_out_path.clone()
                } else {
                    workspace_root.join(&temp_out_path)
                };

                let input_json = serde_json::json!({
                    "input_path": snapshot_path.display().to_string(),
                    "output_path": output_abs.display().to_string(),
                    "config_path": pinned_config.display().to_string(),
                    "theme": "default",
                    "svg_id": svg_id,
                    "seed": seed,
                    "width": 800,
                    "height": 600,
                    "background_color": "white",
                    "browser_executable": render_probe.browser_executable.display().to_string(),
                })
                .to_string();

                let Some(script_path) = seeded_script.as_ref() else {
                    return Err(upstream_svg_failure_with_cleanup(
                        &temp_out_path,
                        "seeded renderer script not available".to_string(),
                    ));
                };

                let mut cmd = Command::new("node");
                cmd.arg(script_path)
                    .current_dir(&node_cwd)
                    .env(
                        "PUPPETEER_EXECUTABLE_PATH",
                        &render_probe.browser_executable,
                    )
                    .env("PUPPETEER_BROWSER", "chrome")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit());
                let mut child = match spawn_timeout_managed_child(&mut cmd) {
                    Ok(child) => child,
                    Err(err) => {
                        return Err(upstream_svg_failure_with_cleanup(
                            &temp_out_path,
                            format!(
                                "failed to spawn seeded upstream svg renderer for {}: {err}",
                                mmd_path.display()
                            ),
                        ));
                    }
                };
                if let Some(mut stdin) = child.stdin.take() {
                    let _ = stdin.write_all(input_json.as_bytes());
                }
                wait_with_timeout(&mut child, per_chart_timeout)
            } else {
                let mut cmd = Command::new("node");
                cmd.arg(mmdc)
                    .arg("-i")
                    .arg(snapshot_path)
                    .arg("-o")
                    .arg(&temp_out_path)
                    .arg("-t")
                    .arg("default")
                    .arg("-p")
                    .arg(puppeteer_config)
                    .env(
                        "PUPPETEER_EXECUTABLE_PATH",
                        &render_probe.browser_executable,
                    )
                    .env("PUPPETEER_BROWSER", "chrome");

                // Stabilize Rough.js output across runs. Mermaid uses Rough.js for many "classic look"
                // shapes too (often with `roughness: 0`), but the stroke control points still depend on
                // `random()` via `divergePoint`. Pin `handDrawnSeed` for reproducible upstream SVG
                // baselines.
                let pinned_config = workspace_root
                    .join("tools")
                    .join("mermaid-cli")
                    .join("mermaid-config.json");
                cmd.arg("-c").arg(pinned_config);

                // Gantt rendering depends on the page width (`parentElement.offsetWidth`). In a
                // headless Rust context we default to the Mermaid fallback width (1200) when no DOM
                // width is available. Use the same page width for upstream baselines so parity diffs
                // remain meaningful.
                if diagram == "gantt" {
                    cmd.arg("-w").arg("1200");
                }

                cmd.arg("--svgId").arg(svg_id);
                cmd.stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit());

                let child = spawn_timeout_managed_child(&mut cmd);
                match child {
                    Ok(mut child) => wait_with_timeout(&mut child, per_chart_timeout),
                    Err(err) => Err(err),
                }
            };

            match status {
                Ok(status) if status.success() => validate_upstream_svg_temp(&temp_out_path)
                    .map(|()| PendingUpstreamSvg {
                        temp_path: temp_out_path.clone(),
                        out_path,
                    })
                    .map_err(|err| {
                        upstream_svg_failure_with_cleanup(
                            &temp_out_path,
                            format!(
                                "mmdc output validation failed for {}: {err}",
                                mmd_path.display()
                            ),
                        )
                    }),
                Ok(status) => Err(upstream_svg_failure_with_cleanup(
                    &temp_out_path,
                    format!(
                        "mmdc failed for {} (exit={})",
                        mmd_path.display(),
                        status.code().unwrap_or(-1)
                    ),
                )),
                Err(err) => Err(upstream_svg_failure_with_cleanup(
                    &temp_out_path,
                    format!("mmdc failed for {}: {err}", mmd_path.display()),
                )),
            }
        });

        let mut pending = Vec::with_capacity(render_results.len());
        let mut failures = Vec::new();
        for result in render_results {
            match result {
                Ok(rendered) => pending.push(rendered),
                Err(failure) => failures.push(failure),
            }
        }
        if !failures.is_empty() {
            failures.extend(cleanup_pending_upstream_svg_temps(&pending));
            let message = failures.join("\n");
            let _ = fs::write(&failures_path, &message);
            return Err(XtaskError::UpstreamSvgFailed(message));
        }

        let family_lock =
            match use_or_acquire_upstream_svg_family_lock(&out_dir, external_family_lock) {
                Ok(lock) => lock,
                Err(err) => {
                    let message = with_batch_cleanup_errors(
                        err.to_string(),
                        cleanup_pending_upstream_svg_temps(&pending),
                    );
                    let _ = fs::write(&failures_path, &message);
                    return Err(XtaskError::UpstreamSvgFailed(message));
                }
            };
        if let Err(err) = ensure_fresh_upstream_svg_output_is_empty(&out_dir, fresh_output) {
            let message = with_batch_cleanup_errors(
                err.to_string(),
                cleanup_pending_upstream_svg_temps(&pending),
            );
            return Err(XtaskError::UpstreamSvgFailed(message));
        }
        let final_scope =
            match crate::cmd::preflight_upstream_svg_provenance_write_under_family_lock(
                &out_dir,
                full_generation,
                fresh_output,
                &render_probe.render_environment,
            ) {
                Ok(scope) => scope,
                Err(err) => {
                    let message = with_batch_cleanup_errors(
                        err.to_string(),
                        cleanup_pending_upstream_svg_temps(&pending),
                    );
                    let _ = fs::write(&failures_path, &message);
                    return Err(XtaskError::UpstreamSvgFailed(message));
                }
            };
        if final_scope != crate::cmd::UpstreamSvgProvenanceWriteScope::Requested {
            let message = with_batch_cleanup_errors(
                format!(
                    "upstream SVG provenance changed while rendering {diagram}; rerun generation"
                ),
                cleanup_pending_upstream_svg_temps(&pending),
            );
            let _ = fs::write(&failures_path, &message);
            return Err(XtaskError::UpstreamSvgFailed(message));
        }
        if let Err(err) = fixture_snapshots.validate_live_selection_and_hashes() {
            let message = with_batch_cleanup_errors(
                err.to_string(),
                cleanup_pending_upstream_svg_temps(&pending),
            );
            let _ = fs::write(&failures_path, &message);
            return Err(XtaskError::UpstreamSvgFailed(message));
        }
        let verified_environment = match render_probe.verified_render_environment() {
            Ok(environment) => environment,
            Err(err) => {
                let message = with_batch_cleanup_errors(
                    err.to_string(),
                    cleanup_pending_upstream_svg_temps(&pending),
                );
                let _ = fs::write(&failures_path, &message);
                return Err(XtaskError::UpstreamSvgFailed(message));
            }
        };

        let commit = || {
            fixture_snapshots
                .validate_live_selection_and_hashes()
                .map_err(|err| err.to_string())?;
            crate::cmd::write_upstream_svg_provenance(
                crate::cmd::UpstreamSvgProvenanceWriteRequest {
                    diagram,
                    fixtures_dir: &fixtures_dir,
                    out_dir: &out_dir,
                    generated_fixtures: fixture_snapshots.renderable(),
                    excluded_fixtures: fixture_snapshots.excluded(),
                    full_generation,
                    fresh_output,
                    render_environment: verified_environment,
                },
                || fixture_snapshots.validate_live_selection_and_hashes(),
            )
            .map_err(|err| err.to_string())
        };
        let promotion = promote_upstream_svg_batch(&pending, &excluded_svg_paths, commit);
        drop(family_lock);
        let snapshot_cleanup = fixture_snapshots.cleanup();
        match promotion {
            Ok(()) => {
                if let Err(cleanup) = snapshot_cleanup {
                    eprintln!("warning: {cleanup}");
                }
                Ok(())
            }
            Err(message) => {
                let message = match snapshot_cleanup {
                    Ok(()) => message,
                    Err(cleanup) => format!("{message}; {cleanup}"),
                };
                let _ = fs::write(&failures_path, &message);
                Err(XtaskError::UpstreamSvgFailed(message))
            }
        }
    }

    let generation_context = UpstreamSvgGenerationContext {
        workspace_root: &workspace_root,
        fixtures_root: &fixtures_root,
        out_root: &out_root,
        mmdc: &mmdc,
        puppeteer_config: &puppeteer_config,
        render_probe: &render_probe,
        jobs,
        fresh_output,
        external_family_lock,
    };

    if diagram != "all" {
        let selected_diagram = selected_diagrams.first().copied().ok_or_else(|| {
            XtaskError::UpstreamSvgFailed(format!(
                "no upstream SVG diagram remained selected for {diagram}"
            ))
        })?;
        return run_one(&generation_context, selected_diagram, filter);
    }

    let mut failures = Vec::new();
    for diagram in selected_diagrams {
        if let Err(err) = run_one(&generation_context, diagram, filter) {
            failures.push(format!("{diagram}: {err}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
    }
}

const REQUIREMENT_FONT_PRECEDENCE_FIXTURE: &str = "stress_requirement_font_size_precedence_001";

fn upstream_svg_check_dom_mode(
    diagram: &str,
    fixture: &str,
    check_dom: bool,
    requested_mode: svgdom::DomMode,
) -> Option<svgdom::DomMode> {
    if diagram == "requirement" && fixture == REQUIREMENT_FONT_PRECEDENCE_FIXTURE {
        return Some(svgdom::DomMode::Strict);
    }
    if check_dom {
        return Some(requested_mode);
    }
    if matches!(
        diagram,
        "state"
            | "gitgraph"
            | "gantt"
            | "er"
            | "class"
            | "requirement"
            | "block"
            | "mindmap"
            | "architecture"
    ) {
        return Some(svgdom::DomMode::Structure);
    }
    None
}

pub(crate) fn check_upstream_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "er".to_string();
    let mut filter: Option<String> = None;
    let mut install: bool = false;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "strict".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--install" => install = true,
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "strict".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let fixtures_root = crate::cmd::fixtures_root();
    let selected_diagrams =
        select_upstream_svg_diagrams(&diagram, &fixtures_root, filter.as_deref())?;
    let baseline_root = fixtures_root.join("upstream-svgs");
    let out_root = create_upstream_svg_check_output_root(&crate::cmd::target_root())?;

    let mut gen_args: Vec<String> = vec![
        "--diagram".to_string(),
        diagram.clone(),
        "--out".to_string(),
        out_root.to_string_lossy().to_string(),
        "--fresh-output".to_string(),
    ];
    if let Some(f) = &filter {
        gen_args.push("--filter".to_string());
        gen_args.push(f.clone());
    }
    if install {
        gen_args.push("--install".to_string());
    }

    gen_upstream_svgs(gen_args)?;
    let current_selection =
        select_upstream_svg_diagrams(&diagram, &fixtures_root, filter.as_deref())?;
    if current_selection != selected_diagrams {
        return Err(XtaskError::UpstreamSvgFailed(
            "upstream SVG family selection changed while running the fresh baseline check"
                .to_string(),
        ));
    }

    struct UpstreamSvgCheck<'a> {
        baseline_root: &'a Path,
        out_root: &'a Path,
        diagram: &'a str,
        filter: Option<&'a str>,
        check_dom: bool,
        dom_mode: svgdom::DomMode,
        dom_decimals: u32,
    }

    fn check_one(ctx: UpstreamSvgCheck<'_>) -> Result<(), XtaskError> {
        let UpstreamSvgCheck {
            baseline_root,
            out_root,
            diagram,
            filter,
            check_dom,
            dom_mode,
            dom_decimals,
        } = ctx;
        let fixtures_dir = crate::cmd::fixtures_root().join(diagram);
        let baseline_dir = baseline_root.join(diagram);
        let out_dir = out_root.join(diagram);
        let _baseline_family_lock = crate::cmd::acquire_upstream_svg_family_lock(&baseline_dir)?;
        let provenance = crate::cmd::load_upstream_svg_provenance(
            diagram,
            &fixtures_dir,
            &baseline_dir,
            filter.is_none(),
        )?;
        let generated_provenance = crate::cmd::load_upstream_svg_provenance(
            diagram,
            &fixtures_dir,
            &out_dir,
            filter.is_none(),
        )?;
        provenance.require_same_generated_environment(&generated_provenance)?;

        let fixture_files = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, filter, false);
        let (mmd_files, excluded_fixtures) =
            partition_upstream_svg_fixtures(diagram, fixture_files)?;
        let mut mismatches: Vec<String> = Vec::new();
        for (fixture_path, reason) in &excluded_fixtures {
            let Some(stem) = fixture_path.file_stem().and_then(|stem| stem.to_str()) else {
                mismatches.push(format!(
                    "invalid fixture filename {}",
                    fixture_path.display()
                ));
                continue;
            };
            let baseline_path = baseline_dir.join(format!("{stem}.svg"));
            let generated_path = out_dir.join(format!("{stem}.svg"));
            if let Err(err) =
                provenance.validate_excluded_fixture(fixture_path, reason, &baseline_path)
            {
                mismatches.push(err);
            }
            if let Err(err) = generated_provenance.validate_excluded_fixture(
                fixture_path,
                reason,
                &generated_path,
            ) {
                mismatches.push(err);
            }
        }

        if mmd_files.is_empty() {
            if !excluded_fixtures.is_empty() {
                println!(
                    "skipped {} upstream svg check fixture(s) for {diagram}: excluded by baseline policy",
                    excluded_fixtures.len()
                );
                return if mismatches.is_empty() {
                    Ok(())
                } else {
                    Err(XtaskError::UpstreamSvgFailed(mismatches.join("\n")))
                };
            }
            return Err(XtaskError::UpstreamSvgFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )));
        }

        for mmd_path in mmd_files {
            let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
                mismatches.push(format!("invalid fixture filename {}", mmd_path.display()));
                continue;
            };

            let baseline_path = baseline_dir.join(format!("{stem}.svg"));
            let out_path = out_dir.join(format!("{stem}.svg"));

            if let Err(err) = provenance.validate_fixture(&mmd_path, &baseline_path) {
                mismatches.push(err);
                continue;
            }

            let baseline_svg = match fs::read_to_string(&baseline_path) {
                Ok(v) => v,
                Err(err) => {
                    mismatches.push(format!(
                        "missing baseline svg: {} ({err})",
                        baseline_path.display()
                    ));
                    continue;
                }
            };
            let out_svg = match fs::read_to_string(&out_path) {
                Ok(v) => v,
                Err(err) => {
                    mismatches.push(format!(
                        "missing generated svg: {} ({err})",
                        out_path.display()
                    ));
                    continue;
                }
            };

            if let Some(mode) = upstream_svg_check_dom_mode(diagram, stem, check_dom, dom_mode) {
                let a = match svgdom::dom_signature(&baseline_svg, mode, dom_decimals) {
                    Ok(v) => v,
                    Err(err) => {
                        mismatches.push(format!(
                            "{diagram}/{stem}: baseline dom parse failed: {err}"
                        ));
                        continue;
                    }
                };
                let b = match svgdom::dom_signature(&out_svg, mode, dom_decimals) {
                    Ok(v) => v,
                    Err(err) => {
                        mismatches.push(format!(
                            "{diagram}/{stem}: generated dom parse failed: {err}"
                        ));
                        continue;
                    }
                };
                if a != b {
                    mismatches.push(format!("{diagram}/{stem}: dom differs from baseline"));
                }
            } else if baseline_svg != out_svg {
                mismatches.push(format!("{diagram}/{stem}: output differs from baseline"));
            }
        }

        if mismatches.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::UpstreamSvgFailed(mismatches.join("\n")))
        }
    }

    let filter = filter.as_deref();
    let parsed_dom_mode = svgdom::DomMode::parse(&dom_mode);
    let mut failures: Vec<String> = Vec::new();
    for target in selected_diagrams {
        if let Err(err) = check_one(UpstreamSvgCheck {
            baseline_root: &baseline_root,
            out_root: &out_root,
            diagram: target,
            filter,
            check_dom,
            dom_mode: parsed_dom_mode,
            dom_decimals,
        }) {
            failures.push(format!("{target}: {err}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::UpstreamSvgFailed(failures.join("\n")))
    }
}

fn ensure_upstream_svg_render_environment_probe_script() -> Result<PathBuf, XtaskError> {
    const JS: &str = r#"
const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');
const url = require('url');
const { createRequire } = require('module');

const requireFromCwd = createRequire(path.join(process.cwd(), 'package.json'));
const puppeteer = requireFromCwd('puppeteer');
function findPackageRoot(entryPath, expectedName) {
  let current = path.dirname(entryPath);
  while (true) {
    const packagePath = path.join(current, 'package.json');
    if (fs.existsSync(packagePath)) {
      const manifest = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
      if (manifest.name === expectedName) return current;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      throw new Error(`unable to locate ${expectedName} package root from ${entryPath}`);
    }
    current = parent;
  }
}
function packageTreeSha256(root) {
  const entries = [];
  function visit(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const fullPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(fullPath);
      } else if (entry.isFile()) {
        entries.push({
          fullPath,
          relativePath: path.relative(root, fullPath).split(path.sep).join('/'),
        });
      } else {
        throw new Error(`unsupported filesystem entry in runtime package: ${fullPath}`);
      }
    }
  }
  visit(root);
  entries.sort((left, right) =>
    left.relativePath < right.relativePath ? -1 : left.relativePath > right.relativePath ? 1 : 0
  );
  const hash = crypto.createHash('sha256');
  for (const entry of entries) {
    hash.update(entry.relativePath, 'utf8');
    hash.update(Buffer.from([0]));
    hash.update(fs.readFileSync(entry.fullPath));
    hash.update(Buffer.from([0]));
  }
  return hash.digest('hex');
}
const mermaidCliEntryPath = requireFromCwd.resolve('@mermaid-js/mermaid-cli');
const mermaidCliRoot = findPackageRoot(mermaidCliEntryPath, '@mermaid-js/mermaid-cli');
const requireFromMermaidCli = createRequire(path.join(mermaidCliRoot, 'src', 'cli.js'));
const mermaidPackagePath = requireFromMermaidCli.resolve('mermaid/package.json');
const mermaidRoot = path.dirname(mermaidPackagePath);
const mermaidHtmlPath = path.join(mermaidCliRoot, 'dist', 'index.html');
const mermaidEsmPath = path.join(mermaidRoot, 'dist', 'mermaid.esm.mjs');
const mermaidIifePath = path.join(mermaidRoot, 'dist', 'mermaid.js');
const puppeteerRoot = findPackageRoot(requireFromCwd.resolve('puppeteer'), 'puppeteer');
const puppeteerPackagePath = path.join(puppeteerRoot, 'package.json');
const FONT_PROBE_REVISION = 'mermaid-font-probe-v1';

function normalizeVersionText(value, runtime) {
  const match = String(value || '').trim().match(/^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/);
  if (!match) {
    throw new Error(`${runtime} info showInfo returned an invalid version: ${JSON.stringify(value)}`);
  }
  return match[1];
}

async function renderInfoVersion(browser, runtime) {
  const page = await browser.newPage();
  try {
    await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
    if (runtime === 'iife') {
      await page.addScriptTag({ path: mermaidIifePath });
    }

    const version = await page.evaluate(
      async ({ runtime, mermaidEsmUrl }) => {
        const mermaid = runtime === 'esm'
          ? (await import(mermaidEsmUrl)).default
          : globalThis.mermaid;
        if (!mermaid) {
          throw new Error(`missing Mermaid ${runtime} runtime`);
        }

        mermaid.initialize({ startOnLoad: false });
        const container = document.getElementById('container') || document.body;
        const rendered = await mermaid.render(`merman-${runtime}-version-probe`, 'info showInfo', container);
        const svg = typeof rendered === 'string' ? rendered : rendered && rendered.svg;
        if (typeof svg !== 'string') {
          throw new Error(`Mermaid ${runtime} info probe returned no SVG`);
        }

        const documentNode = new DOMParser().parseFromString(svg, 'image/svg+xml');
        const versionNode = documentNode.querySelector('text.version');
        return versionNode && versionNode.textContent;
      },
      {
        runtime,
        mermaidEsmUrl: url.pathToFileURL(mermaidEsmPath).href,
      }
    );
    return normalizeVersionText(version, runtime);
  } finally {
    await page.close();
  }
}

async function fingerprintFonts(browser) {
  const page = await browser.newPage();
  try {
    await page.setViewport({ width: 800, height: 600, deviceScaleFactor: 1 });
    await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
    const payload = await page.evaluate(async () => {
      const finiteMetric = (value) =>
        Number.isFinite(value) ? Number(value).toFixed(6) : null;
      if (document.fonts && document.fonts.ready) {
        await document.fonts.ready;
      }

      const samples = [
        ['latin', 'Merman AVWxyz 0123456789 -> — ()[]{}'],
        ['cjk', '汉字測試かなカナ한글'],
        ['complex', 'العربية हिन्दी 😀🧭'],
      ];
      const fontFamilies = [
        '"trebuchet ms", verdana, arial, sans-serif',
        '"Courier New", courier, monospace',
        'serif',
        'sans-serif',
        'monospace',
      ];
      const svgNamespace = 'http://www.w3.org/2000/svg';
      const svg = document.createElementNS(svgNamespace, 'svg');
      svg.setAttribute('width', '2000');
      svg.setAttribute('height', '200');
      svg.style.position = 'absolute';
      svg.style.left = '-10000px';
      svg.style.top = '0';
      document.body.appendChild(svg);

      const canvas = document.createElement('canvas');
      const context = canvas.getContext('2d');
      if (!context) {
        throw new Error('2D canvas context is unavailable');
      }

      const measurements = [];
      for (const fontFamily of fontFamilies) {
        for (const [sampleId, sampleText] of samples) {
          const textNode = document.createElementNS(svgNamespace, 'text');
          textNode.setAttribute('font-family', fontFamily);
          textNode.setAttribute('font-size', '16px');
          textNode.setAttribute('font-weight', '400');
          textNode.textContent = sampleText;
          svg.appendChild(textNode);

          const bbox = textNode.getBBox();
          const clientRect = textNode.getBoundingClientRect();
          const computedLength = textNode.getComputedTextLength();
          context.font = `normal 400 16px ${fontFamily}`;
          const canvasMetrics = context.measureText(sampleText);
          measurements.push({
            font_family: fontFamily,
            sample_id: sampleId,
            svg_bbox: {
              x: finiteMetric(bbox.x),
              y: finiteMetric(bbox.y),
              width: finiteMetric(bbox.width),
              height: finiteMetric(bbox.height),
            },
            svg_client_rect: {
              width: finiteMetric(clientRect.width),
              height: finiteMetric(clientRect.height),
            },
            svg_computed_text_length: finiteMetric(computedLength),
            canvas: {
              width: finiteMetric(canvasMetrics.width),
              actual_bounding_box_ascent: finiteMetric(canvasMetrics.actualBoundingBoxAscent),
              actual_bounding_box_descent: finiteMetric(canvasMetrics.actualBoundingBoxDescent),
              actual_bounding_box_left: finiteMetric(canvasMetrics.actualBoundingBoxLeft),
              actual_bounding_box_right: finiteMetric(canvasMetrics.actualBoundingBoxRight),
            },
          });
          textNode.remove();
        }
      }
      svg.remove();

      return {
        viewport: { width: 800, height: 600, device_scale_factor: 1 },
        measurements,
      };
    });

    return crypto.createHash('sha256').update(JSON.stringify(payload)).digest('hex');
  } finally {
    await page.close();
  }
}

(async () => {
  let browser;
  try {
    browser = await puppeteer.launch({
      browser: 'chrome',
      headless: 'shell',
      detached: false,
      args: ['--no-sandbox', '--disable-setuid-sandbox', '--allow-file-access-from-files'],
    });
    const browserProcess = browser.process();
    if (!browserProcess || !browserProcess.spawnfile) {
      throw new Error('Puppeteer did not expose the launched browser executable');
    }
    const browserExecutable = fs.realpathSync.native(browserProcess.spawnfile);

    const session = await browser.target().createCDPSession();
    const browserVersion = await session.send('Browser.getVersion');
    await session.detach();
    const separator = String(browserVersion.product || '').indexOf('/');
    if (separator <= 0) {
      throw new Error(`CDP returned an invalid browser product: ${JSON.stringify(browserVersion.product)}`);
    }

    const output = {
      render_environment: {
        browser: {
          product: browserVersion.product.slice(0, separator),
          version: browserVersion.product.slice(separator + 1),
          revision: String(browserVersion.revision || ''),
        },
        puppeteer: {
          version: JSON.parse(fs.readFileSync(puppeteerPackagePath, 'utf8')).version,
        },
        operating_system: {
          platform: os.platform(),
          arch: os.arch(),
          release: os.release(),
        },
        mermaid_runtime: {
          esm_version: await renderInfoVersion(browser, 'esm'),
          iife_version: await renderInfoVersion(browser, 'iife'),
          mermaid_package_sha256: packageTreeSha256(mermaidRoot),
          mermaid_cli_package_sha256: packageTreeSha256(mermaidCliRoot),
        },
        font_probe: {
          revision: FONT_PROBE_REVISION,
          sha256: await fingerprintFonts(browser),
        },
      },
      browser_executable: browserExecutable,
      runtime_package_roots: {
        mermaid: mermaidRoot,
        mermaid_cli: mermaidCliRoot,
      },
    };
    process.stdout.write(JSON.stringify(output));
  } finally {
    if (browser) {
      await browser.close();
    }
  }
})().catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
"#;

    ensure_content_addressed_js_script(
        &crate::cmd::target_root().join("xtask-js"),
        "probe-upstream-svg-render-environment",
        JS,
    )
}

pub(crate) fn ensure_seeded_upstream_svg_renderer_script() -> Result<PathBuf, XtaskError> {
    const JS: &str = r#"
const fs = require('fs');
const path = require('path');
const url = require('url');
const { createRequire } = require('module');
const requireFromCwd = createRequire(path.join(process.cwd(), 'package.json'));
const puppeteer = requireFromCwd('puppeteer');
function findPackageRoot(entryPath, expectedName) {
  let current = path.dirname(entryPath);
  while (true) {
    const packagePath = path.join(current, 'package.json');
    if (fs.existsSync(packagePath)) {
      const manifest = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
      if (manifest.name === expectedName) return current;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      throw new Error(`unable to locate ${expectedName} package root from ${entryPath}`);
    }
    current = parent;
  }
}
const mermaidCliEntryPath = requireFromCwd.resolve('@mermaid-js/mermaid-cli');
const mermaidCliRoot = findPackageRoot(mermaidCliEntryPath, '@mermaid-js/mermaid-cli');
const requireFromMermaidCli = createRequire(path.join(mermaidCliRoot, 'src', 'cli.js'));
const mermaidPackagePath = requireFromMermaidCli.resolve('mermaid/package.json');
const mermaidRoot = path.dirname(mermaidPackagePath);

const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const inputPath = String(input.input_path || '');
const outputPath = String(input.output_path || '');
const configPath = String(input.config_path || '');
const theme = String(input.theme || 'default');
const svgId = String(input.svg_id || 'diagram');
const seedStr = String((input.seed ?? 1));
const width = Number(input.width || 800);
const height = Number(input.height || 600);
const backgroundColor = String(input.background_color || 'white');
const browserExecutable = String(input.browser_executable || '');
const debug = process.env.MERMAN_SEEDED_UPSTREAM_SVG_DEBUG === '1';

if (!inputPath || !outputPath || !configPath || !browserExecutable) {
  console.error('missing required input/output/config/browser executable path');
  process.exit(2);
}

const mermaidHtmlPath = path.join(mermaidCliRoot, 'dist', 'index.html');
const mermaidIifePath = path.join(mermaidRoot, 'dist', 'mermaid.js');
const zenumlIifePath = path.join(process.cwd(), 'node_modules', '@mermaid-js', 'mermaid-zenuml', 'dist', 'mermaid-zenuml.js');

(async () => {
  const code = fs.readFileSync(inputPath, 'utf8');
  const cfg = JSON.parse(fs.readFileSync(configPath, 'utf8'));

  const launchOpts = {
    browser: 'chrome',
    executablePath: browserExecutable,
    headless: 'shell',
    detached: false,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--allow-file-access-from-files'],
  };
  const browser = await puppeteer.launch(launchOpts);
  const page = await browser.newPage();
  if (process.env.MERMAN_SEEDED_UPSTREAM_SVG_DEBUG === '1') {
    page.on('console', (msg) => {
      if (!msg || typeof msg.type !== 'function') return;
      const ty = msg.type();
      if (ty === 'error' || ty === 'warning') {
        console.error(`[browser:console.${ty}] ${msg.text()}`);
      }
    });
    page.on('pageerror', (err) => {
      console.error(`[browser:pageerror] ${err && err.stack ? err.stack : String(err)}`);
    });
  }

  await page.evaluateOnNewDocument((seedStr) => {
    const mask64 = (1n << 64n) - 1n;
    let state = (BigInt(seedStr) & mask64);
    if (state === 0n) state = 1n;

    function nextU64() {
      let x = state;
      x ^= (x >> 12n);
      x ^= (x << 25n) & mask64;
      x ^= (x >> 27n);
      state = x;
      return (x * 0x2545F4914F6CDD1Dn) & mask64;
    }

    function nextF64() {
      const u = nextU64() >> 11n;
      return Number(u) / 9007199254740992; // 2^53
    }

    Math.random = nextF64;

    if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === 'function') {
      const orig = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
      globalThis.crypto.getRandomValues = (arr) => {
        if (!arr || typeof arr.length !== 'number') {
          return orig(arr);
        }
        // Fill the underlying bytes so behavior is consistent for Uint8/16/32 arrays.
        try {
          const bytes = new Uint8Array(arr.buffer, arr.byteOffset || 0, arr.byteLength || 0);
          for (let i = 0; i < bytes.length; i++) {
            bytes[i] = Math.floor(nextF64() * 256);
          }
          return arr;
        } catch (e) {
          // Fall back to original behavior if this isn't a typed array.
          return orig(arr);
        }
      };
    }
  }, seedStr);

  await page.setViewport({ width: Math.max(1, width), height: Math.max(1, height), deviceScaleFactor: 1 });
  await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
  await Promise.all([
    page.addScriptTag({ path: mermaidIifePath }),
    page.addScriptTag({ path: zenumlIifePath }),
  ]);

  const svg = await page.evaluate(async ({ code, cfg, theme, svgId, width, debug }) => {
    const mermaid = globalThis.mermaid;
    if (!mermaid) throw new Error('global mermaid instance not found (mermaid.js)');

    if (document.fonts && typeof document.fonts[Symbol.iterator] === 'function') {
      await Promise.all(Array.from(document.fonts, (font) => font.load()));
    }

    // Match mermaid-cli behavior: register external diagrams and layout loaders.
    const zenuml = globalThis['mermaid-zenuml'];
    if (zenuml && typeof mermaid.registerExternalDiagrams === 'function') {
      await mermaid.registerExternalDiagrams([zenuml]);
    }
    const elkLayouts = globalThis.elkLayouts;
    if (elkLayouts && typeof mermaid.registerLayoutLoaders === 'function') {
      mermaid.registerLayoutLoaders(elkLayouts);
    }

    mermaid.initialize(Object.assign({ startOnLoad: false, theme }, cfg));

    const container = document.getElementById('container') || document.body;
    container.innerHTML = '';
    container.style.width = `${Math.max(1, Number(width) || 1)}px`;

    // Surface parse errors early; some Mermaid failures otherwise only manifest as a missing `svg`.
    if (typeof mermaid.parse === 'function') {
      try {
        await mermaid.parse(code);
      } catch (err) {
        if (!debug) throw err;
        return {
          ok: false,
          stage: 'parse',
          error: String(err && err.message ? err.message : err),
          stack: String(err && err.stack ? err.stack : ''),
        };
      }
    }

    async function tryRenderViaMermaidRender() {
      if (typeof mermaid.render !== 'function') return undefined;
      const rendered = await mermaid.render(svgId, code, container);
      let svg =
        typeof rendered === 'string'
          ? rendered
          : Array.isArray(rendered)
            ? rendered[0]
            : rendered && rendered.svg;
      if (typeof svg !== 'string' && rendered != null) {
        const asStr = String(rendered);
        if (asStr.trim().startsWith('<svg')) {
          svg = asStr;
        }
      }
      if (typeof svg === 'string') return svg;
      const domSvg = container.querySelector && container.querySelector('svg');
      if (domSvg && typeof domSvg.outerHTML === 'string' && domSvg.outerHTML.trim().startsWith('<svg')) {
        return domSvg.outerHTML;
      }
      return undefined;
    }

    async function tryRenderViaMermaidApi() {
      const api = mermaid.mermaidAPI;
      if (!api || typeof api.render !== 'function') return undefined;
      return await new Promise((resolve, reject) => {
        try {
          api.render(svgId, code, (svgCode) => resolve(svgCode), container);
        } catch (err) {
          reject(err);
        }
      });
    }

    const svgText = (await tryRenderViaMermaidRender()) ?? (await tryRenderViaMermaidApi());
    if (typeof svgText !== 'string') {
      if (!debug) {
        throw new Error('mermaid.render returned no svg output');
      }
      return {
        ok: false,
        stage: 'render',
        svgTextType: typeof svgText,
        containerHtmlLen: typeof container.innerHTML === 'string' ? container.innerHTML.length : -1,
      };
    }

    container.innerHTML = svgText;
    const svgEl = container.getElementsByTagName?.('svg')?.[0];
    if (!svgEl) {
      if (debug) {
        return { ok: true, stage: 'no-svg-el', svgTextLen: svgText.length };
      }
      return svgText;
    }

    // Mirror mermaid-cli SVG output shape (XMLSerializer), so outputs are valid XML.
    // eslint-disable-next-line no-undef
    const xmlSerializer = new XMLSerializer();
    const xml = xmlSerializer.serializeToString(svgEl);
    if (debug) {
      return { ok: true, stage: 'ok', svgTextLen: svgText.length, serializedLen: xml.length };
    }
    return xml;
  }, { code, cfg, theme, svgId, width, debug });

  if (debug) {
    if (typeof svg !== 'string') {
      console.error(JSON.stringify(svg, null, 2));
      process.exit(1);
    }
    console.error(`[debug] expected diagnostics object, got svg string len=${svg.length}`);
    process.exit(1);
  }

  function ensureSvgBackgroundColor(svgText, bg) {
    if (typeof svgText !== 'string') {
      throw new Error(`expected svg string from mermaid.render, got ${typeof svgText}`);
    }
    if (!bg) return svgText;
    if (svgText.includes('background-color:')) return svgText;
    const m = svgText.match(/<svg\b[^>]*\bstyle="([^"]*)"/);
    if (m) {
      const raw = m[1] || '';
      let next = raw.trim();
      if (next.length > 0 && !next.trim().endsWith(';')) {
        next += ';';
      }
      next += ` background-color: ${bg};`;
      return svgText.replace(m[0], m[0].replace(raw, next));
    }
    // Fallback: inject a style attr into the root <svg>.
    return svgText.replace(/<svg\b/, `<svg style="background-color: ${bg};"`);
  }

  const svgWithBg = ensureSvgBackgroundColor(svg, backgroundColor);
  fs.writeFileSync(outputPath, svgWithBg, 'utf8');
  await browser.close();
})().catch((err) => {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(1);
});
"#;

    ensure_content_addressed_js_script(
        &crate::cmd::target_root().join("xtask-js"),
        "seeded-upstream-svg-render",
        JS,
    )
}

fn export_svg_fixtures<F>(
    fixtures_dir: &Path,
    out_dir: &Path,
    filter: Option<&str>,
    mut render: F,
) -> Result<(), XtaskError>
where
    F: FnMut(&Path, &str, &str) -> Result<String, String>,
{
    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(fixtures_dir) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if let Some(f) = filter
            && !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
        {
            continue;
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let mut failures: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let svg = match render(&mmd_path, stem, &text) {
            Ok(v) => v,
            Err(err) => {
                failures.push(err);
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.svg"));
        if let Err(err) = fs::write(&out_path, svg) {
            failures.push(format!("failed to write {}: {err}", out_path.display()));
            continue;
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

pub(crate) fn gen_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let out_root = out_root.unwrap_or_else(|| crate::cmd::target_root().join("svgs"));

    let fixtures_dir = crate::cmd::fixtures_root().join("er");
    let out_dir = out_root.join("er");

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let layout_opts = merman_render::LayoutOptions::default();
    export_svg_fixtures(
        &fixtures_dir,
        &out_dir,
        filter.as_deref(),
        |mmd_path, stem, text| {
            let parsed = match futures::executor::block_on(engine.parse_diagram(
                text,
                merman::ParseOptions {
                    suppress_errors: true,
                },
            )) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    return Err(format!("no diagram detected in {}", mmd_path.display()));
                }
                Err(err) => {
                    return Err(format!("parse failed for {}: {err}", mmd_path.display()));
                }
            };

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("layout failed for {}: {err}", mmd_path.display()));
                }
            };

            let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(stem.to_string()),
                ..Default::default()
            };

            let svg = match merman_render::svg::render_er_diagram_svg(
                layout,
                &layouted.semantic,
                &layouted.meta.effective_config,
                layouted.meta.title.as_deref(),
                layout_opts.text_measurer.as_ref(),
                &svg_opts,
            ) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("render failed for {}: {err}", mmd_path.display()));
                }
            };

            Ok(svg)
        },
    )
}

pub(crate) fn gen_debug_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut diagram: String = "class".to_string();
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
            }
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let out_root = out_root.unwrap_or_else(|| crate::cmd::target_root().join("debug-svgs"));

    fn gen_one(out_root: &Path, diagram: &str, filter: Option<&str>) -> Result<(), XtaskError> {
        let (fixtures_dir, out_dir) = match diagram {
            "flowchart" | "flowchart-v2" | "flowchartV2" => (
                crate::cmd::fixtures_root().join("flowchart"),
                out_root.join("flowchart"),
            ),
            "state" | "stateDiagram" | "stateDiagram-v2" | "stateDiagramV2" => (
                crate::cmd::fixtures_root().join("state"),
                out_root.join("state"),
            ),
            "class" | "classDiagram" => (
                crate::cmd::fixtures_root().join("class"),
                out_root.join("class"),
            ),
            "er" | "erDiagram" => (crate::cmd::fixtures_root().join("er"), out_root.join("er")),
            "sequence" => (
                crate::cmd::fixtures_root().join("sequence"),
                out_root.join("sequence"),
            ),
            "info" => (
                crate::cmd::fixtures_root().join("info"),
                out_root.join("info"),
            ),
            "pie" => (
                crate::cmd::fixtures_root().join("pie"),
                out_root.join("pie"),
            ),
            "packet" => (
                crate::cmd::fixtures_root().join("packet"),
                out_root.join("packet"),
            ),
            other => {
                return Err(XtaskError::DebugSvgFailed(format!(
                    "unsupported diagram for debug svg export: {other} (supported: flowchart, state, class, er, sequence, info, pie, packet)"
                )));
            }
        };

        let engine = merman::Engine::new();
        let layout_opts = merman_render::LayoutOptions::default();

        export_svg_fixtures(&fixtures_dir, &out_dir, filter, |mmd_path, _stem, text| {
            let parsed = match futures::executor::block_on(
                engine.parse_diagram(text, merman::ParseOptions::default()),
            ) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    return Err(format!("no diagram detected in {}", mmd_path.display()));
                }
                Err(err) => {
                    return Err(format!("parse failed for {}: {err}", mmd_path.display()));
                }
            };

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("layout failed for {}: {err}", mmd_path.display()));
                }
            };

            let svg = match &layouted.layout {
                merman_render::model::LayoutDiagram::FlowchartV2(layout) => {
                    Ok(merman_render::svg::render_flowchart_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::StateDiagramV2(layout) => {
                    Ok(merman_render::svg::render_state_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::ClassDiagramV2(layout) => {
                    Ok(merman_render::svg::render_class_diagram_v2_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::ErDiagram(layout) => {
                    Ok(merman_render::svg::render_er_diagram_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::SequenceDiagram(layout) => {
                    Ok(merman_render::svg::render_sequence_diagram_debug_svg(
                        layout,
                        &merman_render::svg::SvgRenderOptions::default(),
                    ))
                }
                merman_render::model::LayoutDiagram::InfoDiagram(layout) => {
                    merman_render::svg::render_info_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "info svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::PieDiagram(layout) => {
                    merman_render::svg::render_pie_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "pie svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::PacketDiagram(layout) => {
                    merman_render::svg::render_packet_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "packet svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::TimelineDiagram(layout) => {
                    merman_render::svg::render_timeline_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        layout_opts.text_measurer.as_ref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "timeline svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::JourneyDiagram(layout) => {
                    merman_render::svg::render_journey_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        layouted.meta.title.as_deref(),
                        layout_opts.text_measurer.as_ref(),
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "journey svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                merman_render::model::LayoutDiagram::KanbanDiagram(layout) => {
                    merman_render::svg::render_kanban_diagram_svg(
                        layout,
                        &layouted.semantic,
                        &layouted.meta.effective_config,
                        &merman_render::svg::SvgRenderOptions::default(),
                    )
                    .map_err(|e| {
                        XtaskError::DebugSvgFailed(format!(
                            "kanban svg render failed for {}: {e}",
                            mmd_path.display()
                        ))
                    })
                }
                _ => Err(XtaskError::DebugSvgFailed(format!(
                    "unsupported layout for debug svg export: {} ({})",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ))),
            };

            let svg = match svg {
                Ok(v) => v,
                Err(err) => return Err(err.to_string()),
            };

            Ok(svg)
        })
    }

    let filter = filter.as_deref();
    let diagrams: Vec<&str> = match diagram.as_str() {
        "all" => vec!["flowchart", "state", "class", "er"],
        other => vec![other],
    };

    let mut failures: Vec<String> = Vec::new();
    for d in diagrams {
        if let Err(err) = gen_one(&out_root, d, filter) {
            failures.push(format!("{d}: {err}"));
        }
    }

    if failures.is_empty() {
        return Ok(());
    }

    Err(XtaskError::DebugSvgFailed(failures.join("\n")))
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum DefaultConfigOverrideOp {
    Set,
    Remove,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct DefaultConfigOverride {
    op: DefaultConfigOverrideOp,
    path: Vec<String>,
    #[serde(default)]
    value: Option<JsonValue>,
    #[serde(default, rename = "reason")]
    _reason: Option<String>,
}

fn default_config_overrides_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("default_config_overrides.json")
}

fn read_default_config_overrides(path: &Path) -> Result<Vec<DefaultConfigOverride>, XtaskError> {
    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    Ok(serde_json::from_str(&text)?)
}

fn apply_default_config_overrides(
    root: &mut JsonValue,
    overrides: &[DefaultConfigOverride],
) -> Result<(), XtaskError> {
    for override_entry in overrides {
        apply_default_config_override(root, override_entry)?;
    }
    Ok(())
}

fn apply_default_config_override(
    root: &mut JsonValue,
    override_entry: &DefaultConfigOverride,
) -> Result<(), XtaskError> {
    if override_entry.path.is_empty() {
        return Err(XtaskError::DefaultConfigOverride(
            "override path must not be empty".to_string(),
        ));
    }

    match override_entry.op {
        DefaultConfigOverrideOp::Set => {
            let value = override_entry.value.clone().ok_or_else(|| {
                XtaskError::DefaultConfigOverride(format!(
                    "set override for `{}` is missing value",
                    override_entry.path.join(".")
                ))
            })?;
            set_json_path(root, &override_entry.path, value)
        }
        DefaultConfigOverrideOp::Remove => {
            remove_json_path(root, &override_entry.path);
            Ok(())
        }
    }
}

fn set_json_path(
    root: &mut JsonValue,
    path: &[String],
    value: JsonValue,
) -> Result<(), XtaskError> {
    let mut cur = root;
    for segment in &path[..path.len() - 1] {
        if !cur.is_object() {
            return Err(XtaskError::DefaultConfigOverride(format!(
                "cannot set `{}` through non-object segment `{segment}`",
                path.join(".")
            )));
        }
        let obj = cur.as_object_mut().ok_or_else(|| {
            XtaskError::DefaultConfigOverride(format!(
                "cannot set `{}` through non-object segment `{segment}`",
                path.join(".")
            ))
        })?;
        cur = obj
            .entry(segment.clone())
            .or_insert_with(|| JsonValue::Object(Default::default()));
    }

    let leaf = path.last().expect("path is known non-empty");
    let obj = cur.as_object_mut().ok_or_else(|| {
        XtaskError::DefaultConfigOverride(format!(
            "cannot set `{}` on a non-object parent",
            path.join(".")
        ))
    })?;
    obj.insert(leaf.clone(), value);
    Ok(())
}

fn remove_json_path(root: &mut JsonValue, path: &[String]) {
    let mut cur = root;
    for segment in &path[..path.len() - 1] {
        let Some(obj) = cur.as_object_mut() else {
            return;
        };
        let Some(next) = obj.get_mut(segment) else {
            return;
        };
        cur = next;
    }

    if let Some(obj) = cur.as_object_mut()
        && let Some(leaf) = path.last()
    {
        obj.remove(leaf);
    }
}

fn sort_json_value_keys(value: &mut JsonValue) {
    match value {
        JsonValue::Object(map) => {
            for child in map.values_mut() {
                sort_json_value_keys(child);
            }

            let mut sorted = JsonMap::new();
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort();
            for key in keys {
                if let Some(child) = map.remove(&key) {
                    sorted.insert(key, child);
                }
            }
            *map = sorted;
        }
        JsonValue::Array(items) => {
            for item in items {
                sort_json_value_keys(item);
            }
        }
        _ => {}
    }
}

pub(crate) fn gen_default_config(args: Vec<String>) -> Result<(), XtaskError> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Err(XtaskError::Usage);
    }

    let mut schema_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut overrides_path: Option<PathBuf> = None;
    let mut apply_local_overrides = true;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--schema" => {
                i += 1;
                schema_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--overrides" => {
                i += 1;
                overrides_path = args.get(i).map(PathBuf::from);
                apply_local_overrides = true;
            }
            "--no-local-overrides" => {
                apply_local_overrides = false;
            }
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let schema_path = schema_path.unwrap_or_else(crate::cmd::default_config_schema_path);
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/default_config.json"));

    let schema_text = fs::read_to_string(&schema_path).map_err(|source| XtaskError::ReadFile {
        path: schema_path.display().to_string(),
        source,
    })?;
    let schema_yaml = serde_saphyr::from_str::<JsonValue>(&schema_text)?;

    let Some(mut root_defaults) = extract_defaults(&schema_yaml, &schema_yaml) else {
        return Err(XtaskError::InvalidRef(
            "schema produced no defaults (unexpected)".to_string(),
        ));
    };
    if apply_local_overrides {
        let overrides_path = overrides_path.unwrap_or_else(default_config_overrides_path);
        let overrides = read_default_config_overrides(&overrides_path)?;
        apply_default_config_overrides(&mut root_defaults, &overrides)?;
    }

    sort_json_value_keys(&mut root_defaults);
    let mut pretty = serde_json::to_string_pretty(&root_defaults)?;
    pretty.push('\n');
    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    fs::write(&out_path, pretty).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

pub(crate) fn gen_dompurify_defaults(args: Vec<String>) -> Result<(), XtaskError> {
    let mut src_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--src" => {
                i += 1;
                src_path = args.get(i).map(PathBuf::from);
            }
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let src_path_was_explicit = src_path.is_some();
    let src_path = src_path.unwrap_or_else(|| {
        crate::cmd::dompurify_repo_root()
            .join("dist")
            .join("purify.cjs.js")
    });
    let out_path = out_path
        .unwrap_or_else(|| PathBuf::from("crates/merman-core/src/generated/dompurify_defaults.rs"));

    if !src_path_was_explicit && !src_path.exists() {
        return Err(XtaskError::MissingReference(
            dompurify_reference_checkout_message(&src_path),
        ));
    }

    let src_text = fs::read_to_string(&src_path).map_err(|source| XtaskError::ReadFile {
        path: src_path.display().to_string(),
        source,
    })?;

    let html_tags = extract_frozen_string_array(&src_text, "html$1")?;
    let svg_tags = extract_frozen_string_array(&src_text, "svg$1")?;
    let svg_filters = extract_frozen_string_array(&src_text, "svgFilters")?;
    let mathml_tags = extract_frozen_string_array(&src_text, "mathMl$1")?;

    let html_attrs = extract_frozen_string_array(&src_text, "html")?;
    let svg_attrs = extract_frozen_string_array(&src_text, "svg")?;
    let mathml_attrs = extract_frozen_string_array(&src_text, "mathMl")?;
    let xml_attrs = extract_frozen_string_array(&src_text, "xml")?;

    let default_data_uri_tags =
        extract_add_to_set_string_array(&src_text, "DEFAULT_DATA_URI_TAGS")?;
    let default_uri_safe_attrs =
        extract_add_to_set_string_array(&src_text, "DEFAULT_URI_SAFE_ATTRIBUTES")?;

    let allowed_tags = unique_sorted_lowercase(
        html_tags
            .into_iter()
            .chain(svg_tags)
            .chain(svg_filters)
            .chain(mathml_tags),
    );

    let allowed_attrs = unique_sorted_lowercase(
        html_attrs
            .into_iter()
            .chain(svg_attrs)
            .chain(mathml_attrs)
            .chain(xml_attrs),
    );

    let data_uri_tags = unique_sorted_lowercase(default_data_uri_tags);
    let uri_safe_attrs = unique_sorted_lowercase(default_uri_safe_attrs);

    let out_dir = out_path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let rust = render_dompurify_defaults_rs(
        &allowed_tags,
        &allowed_attrs,
        &uri_safe_attrs,
        &data_uri_tags,
    );
    fs::write(&out_path, rust).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    Ok(())
}

fn dompurify_reference_checkout_message(src_path: &Path) -> String {
    format!(
        "DOMPurify dist is missing at `{}`. Materialize `repo-ref/dompurify` at DOMPurify {DOMPURIFY_BASELINE_VERSION} from `tools/upstreams/REPOS.lock.json`, or pass `--src <purify.cjs.js>` to `gen-dompurify-defaults`.",
        src_path.display()
    )
}

pub(crate) fn render_dompurify_defaults_rs(
    allowed_tags: &[String],
    allowed_attrs: &[String],
    uri_safe_attrs: &[String],
    data_uri_tags: &[String],
) -> String {
    fn render_slice(name: &str, values: &[String]) -> String {
        let mut out = String::new();
        // Keep small slices compact for readability and stable diffs.
        if values.len() <= 8 {
            out.push_str(&format!("pub const {name}: &[&str] = &["));
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("{v:?}"));
            }
            out.push_str("];\n\n");
            return out;
        }
        out.push_str(&format!("pub const {name}: &[&str] = &[\n"));
        for v in values {
            out.push_str(&format!("    {v:?},\n"));
        }
        out.push_str("];\n\n");
        out
    }

    let mut out = String::new();
    out.push_str("// This file is @generated by `cargo run -p xtask -- gen-dompurify-defaults`.\n");
    out.push_str(&format!(
        "// Source: `repo-ref/dompurify/dist/purify.cjs.js` (DOMPurify {DOMPURIFY_BASELINE_VERSION})\n\n"
    ));
    out.push_str(&render_slice("DEFAULT_ALLOWED_TAGS", allowed_tags));
    out.push_str(&render_slice("DEFAULT_ALLOWED_ATTR", allowed_attrs));
    out.push_str(&render_slice("DEFAULT_URI_SAFE_ATTRIBUTES", uri_safe_attrs));
    out.push_str(&render_slice("DEFAULT_DATA_URI_TAGS", data_uri_tags));
    out
}

fn unique_sorted_lowercase<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut set = std::collections::BTreeSet::new();
    for v in values {
        set.insert(v.to_ascii_lowercase());
    }
    set.into_iter().collect()
}

pub(crate) fn gen_flowchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let out_root = out_root.unwrap_or_else(|| crate::cmd::target_root().join("svgs"));

    let fixtures_dir = crate::cmd::fixtures_root().join("flowchart");
    let out_dir = out_root.join("flowchart");

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    export_svg_fixtures(
        &fixtures_dir,
        &out_dir,
        filter.as_deref(),
        |mmd_path, stem, text| {
            let parsed = match futures::executor::block_on(
                engine.parse_diagram(text, merman::ParseOptions::default()),
            ) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    return Err(format!("no diagram detected in {}", mmd_path.display()));
                }
                Err(err) => {
                    return Err(format!("parse failed for {}: {err}", mmd_path.display()));
                }
            };

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("layout failed for {}: {err}", mmd_path.display()));
                }
            };

            let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(stem.to_string()),
                ..Default::default()
            };

            let svg = match merman_render::svg::render_flowchart_v2_svg(
                layout,
                &layouted.semantic,
                &layouted.meta.effective_config,
                layouted.meta.title.as_deref(),
                layout_opts.text_measurer.as_ref(),
                &svg_opts,
            ) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("render failed for {}: {err}", mmd_path.display()));
                }
            };

            Ok(svg)
        },
    )
}

pub(crate) fn gen_state_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let out_root = out_root.unwrap_or_else(|| crate::cmd::target_root().join("svgs"));

    let fixtures_dir = crate::cmd::fixtures_root().join("state");
    let out_dir = out_root.join("state");

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    export_svg_fixtures(
        &fixtures_dir,
        &out_dir,
        filter.as_deref(),
        |mmd_path, stem, text| {
            let parsed = match futures::executor::block_on(
                engine.parse_diagram(text, merman::ParseOptions::default()),
            ) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    return Err(format!("no diagram detected in {}", mmd_path.display()));
                }
                Err(err) => {
                    return Err(format!("parse failed for {}: {err}", mmd_path.display()));
                }
            };

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("layout failed for {}: {err}", mmd_path.display()));
                }
            };

            let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = &layouted.layout
            else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(stem.to_string()),
                ..Default::default()
            };

            let svg = match merman_render::svg::render_state_diagram_v2_svg(
                layout,
                &layouted.semantic,
                &layouted.meta.effective_config,
                layouted.meta.title.as_deref(),
                layout_opts.text_measurer.as_ref(),
                &svg_opts,
            ) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("render failed for {}: {err}", mmd_path.display()));
                }
            };

            Ok(svg)
        },
    )
}

pub(crate) fn gen_class_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let out_root = out_root.unwrap_or_else(|| crate::cmd::target_root().join("svgs"));

    let fixtures_dir = crate::cmd::fixtures_root().join("class");
    let out_dir = out_root.join("class");

    let engine = merman::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    export_svg_fixtures(
        &fixtures_dir,
        &out_dir,
        filter.as_deref(),
        |mmd_path, stem, text| {
            let is_classdiagram_v2_header = merman::preprocess_diagram(text, engine.registry())
                .ok()
                .map(|p| p.code.trim_start().starts_with("classDiagram-v2"))
                .unwrap_or(false);

            let parsed = match futures::executor::block_on(
                engine.parse_diagram(text, merman::ParseOptions::default()),
            ) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    return Err(format!("no diagram detected in {}", mmd_path.display()));
                }
                Err(err) => {
                    return Err(format!("parse failed for {}: {err}", mmd_path.display()));
                }
            };

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("layout failed for {}: {err}", mmd_path.display()));
                }
            };

            let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = &layouted.layout
            else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(stem.to_string()),
                aria_roledescription: is_classdiagram_v2_header.then(|| "classDiagram".to_string()),
                ..Default::default()
            };

            let svg = match merman_render::svg::render_class_diagram_v2_svg(
                layout,
                &layouted.semantic,
                &layouted.meta.effective_config,
                layouted.meta.title.as_deref(),
                layout_opts.text_measurer.as_ref(),
                &svg_opts,
            ) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("render failed for {}: {err}", mmd_path.display()));
                }
            };

            Ok(svg)
        },
    )
}

pub(crate) fn gen_c4_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_root: Option<PathBuf> = None;
    let mut filter: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_root = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let out_root = out_root.unwrap_or_else(|| crate::cmd::target_root().join("svgs"));

    let fixtures_dir = crate::cmd::fixtures_root().join("c4");
    let out_dir = out_root.join("c4");

    // Keep this aligned with `crates/merman-render/tests/layout_snapshots_test.rs` so the
    // `update-layout-snapshots` output matches the test's computed layouts.
    let engine = merman_core::Engine::new();
    let layout_opts = merman_render::LayoutOptions::default();
    export_svg_fixtures(
        &fixtures_dir,
        &out_dir,
        filter.as_deref(),
        |mmd_path, stem, text| {
            let parsed = match futures::executor::block_on(engine.parse_diagram(
                text,
                merman_core::ParseOptions {
                    suppress_errors: true,
                },
            )) {
                Ok(Some(v)) => v,
                Ok(None) => {
                    return Err(format!("no diagram detected in {}", mmd_path.display()));
                }
                Err(err) => {
                    return Err(format!("parse failed for {}: {err}", mmd_path.display()));
                }
            };

            let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("layout failed for {}: {err}", mmd_path.display()));
                }
            };

            let merman_render::model::LayoutDiagram::C4Diagram(layout) = &layouted.layout else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    mmd_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(stem.to_string()),
                ..Default::default()
            };

            let svg = match merman_render::svg::render_c4_diagram_svg(
                layout,
                &layouted.semantic,
                &layouted.meta.effective_config,
                layouted.meta.title.as_deref(),
                layout_opts.text_measurer.as_ref(),
                &svg_opts,
            ) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!("render failed for {}: {err}", mmd_path.display()));
                }
            };

            Ok(svg)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        DOMPURIFY_BASELINE_VERSION, DefaultConfigOverride, DefaultConfigOverrideOp,
        PINNED_MERMAID_CLI_PACKAGE_SHA256, PINNED_MERMAID_PACKAGE_SHA256, PendingUpstreamSvg,
        REQUIREMENT_FONT_PRECEDENCE_FIXTURE, UPSTREAM_SVG_DIAGRAMS, UpstreamSvgRenderProbe,
        UpstreamSvgRuntimePackageRoots, absolutize_workspace_path, apply_default_config_overrides,
        create_upstream_svg_check_output_root, ensure_content_addressed_js_script,
        ensure_fresh_upstream_svg_output_is_empty, map_bounded_in_order,
        parse_gen_upstream_svgs_options, parse_upstream_svg_jobs, partition_upstream_svg_fixtures,
        promote_upstream_svg_batch, read_bounded_child_pipe, render_dompurify_defaults_rs,
        select_upstream_svg_diagrams, sort_json_value_keys, spawn_timeout_managed_child,
        unique_upstream_svg_failure_report_path, unique_upstream_svg_temp_path,
        upstream_svg_check_dom_mode, upstream_svg_filter_matches, upstream_svg_package_tree_sha256,
        use_or_acquire_upstream_svg_family_lock, validate_and_promote_upstream_svg_temp,
        validate_external_upstream_svg_family_lock, validate_mermaid_cli_install,
        validate_upstream_svg_filter_selection, validate_upstream_svg_render_probe,
        wait_with_bounded_output, wait_with_timeout,
    };
    use crate::cmd::{
        acquire_upstream_svg_family_lock, acquire_upstream_svg_family_lock_with_timeout,
    };
    use crate::svgdom::DomMode;
    use serde_json::json;
    use std::fs;
    use std::io::{Cursor, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    fn unique_test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "merman-xtask-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn remove_test_root(root: &Path) {
        let temp_root = fs::canonicalize(std::env::temp_dir()).expect("canonical temp root");
        let test_root = fs::canonicalize(root).expect("canonical test root");
        assert!(test_root.starts_with(&temp_root));
        assert_ne!(test_root, temp_root);
        fs::remove_dir_all(test_root).expect("remove isolated test root");
    }

    fn unique_test_svg_temp_path(root: &Path, out_path: &Path) -> PathBuf {
        let staging_dir = root.join("staging");
        fs::create_dir_all(&staging_dir).expect("create test staging directory");
        unique_upstream_svg_temp_path(&staging_dir, out_path)
    }

    fn write_package_manifest(path: &Path, name: &str, version: &str) {
        fs::create_dir_all(path.parent().expect("package manifest parent"))
            .expect("create package manifest parent");
        fs::write(
            path,
            serde_json::to_vec(&json!({ "name": name, "version": version }))
                .expect("serialize package manifest"),
        )
        .expect("write package manifest");
    }

    fn write_mermaid_cli_install_fixture(root: &Path, cli_version: &str, mermaid_version: &str) {
        fs::create_dir_all(root).expect("create tools root");
        fs::write(
            root.join("package.json"),
            serde_json::to_vec(&json!({
                "name": "merman-upstream-mermaid-cli",
                "private": true,
                "devDependencies": {
                    "@mermaid-js/mermaid-cli": "11.16.0"
                },
                "overrides": {
                    "mermaid": "11.16.0"
                }
            }))
            .expect("serialize tools manifest"),
        )
        .expect("write tools manifest");
        let mermaid_cli_root = root.join("node_modules/@mermaid-js/mermaid-cli");
        let mermaid_cli_entry = mermaid_cli_root.join("src/cli.js");
        fs::create_dir_all(mermaid_cli_entry.parent().expect("CLI entry parent"))
            .expect("create Mermaid CLI package");
        fs::write(
            mermaid_cli_root.join("package.json"),
            serde_json::to_vec(&json!({
                "name": "@mermaid-js/mermaid-cli",
                "version": cli_version,
                "bin": { "mmdc": "./src/cli.js" }
            }))
            .expect("serialize Mermaid CLI package manifest"),
        )
        .expect("write Mermaid CLI package manifest");
        fs::write(&mermaid_cli_entry, "#!/usr/bin/env node\n").expect("write Mermaid CLI entry");
        write_package_manifest(
            &root.join("node_modules/mermaid/package.json"),
            "mermaid",
            mermaid_version,
        );
    }

    #[test]
    fn default_config_overrides_set_and_remove_nested_paths() {
        let mut root = json!({
            "class": { "padding": 5 },
            "flowchart": { "htmlLabels": null },
            "pie": {
                "textPosition": 0.75,
                "donutHole": 0,
                "legendPosition": "right"
            },
            "treeView": { "paddingX": 5 }
        });
        let overrides = vec![
            DefaultConfigOverride {
                op: DefaultConfigOverrideOp::Set,
                path: vec!["class".to_string(), "padding".to_string()],
                value: Some(json!(12)),
                _reason: None,
            },
            DefaultConfigOverride {
                op: DefaultConfigOverrideOp::Set,
                path: vec!["flowchart".to_string(), "htmlLabels".to_string()],
                value: Some(json!(true)),
                _reason: None,
            },
            DefaultConfigOverride {
                op: DefaultConfigOverrideOp::Remove,
                path: vec!["treeView".to_string()],
                value: None,
                _reason: None,
            },
            DefaultConfigOverride {
                op: DefaultConfigOverrideOp::Remove,
                path: vec!["pie".to_string(), "donutHole".to_string()],
                value: None,
                _reason: None,
            },
        ];

        apply_default_config_overrides(&mut root, &overrides).expect("overrides apply");

        assert_eq!(root["class"]["padding"], json!(12));
        assert_eq!(root["flowchart"]["htmlLabels"], json!(true));
        assert!(root.get("treeView").is_none());
        assert!(root["pie"].get("donutHole").is_none());
        assert_eq!(root["pie"]["legendPosition"], json!("right"));
    }

    #[test]
    fn default_config_set_override_creates_missing_objects() {
        let mut root = json!({});
        let overrides = [DefaultConfigOverride {
            op: DefaultConfigOverrideOp::Set,
            path: vec!["sankey".to_string(), "nodeColors".to_string()],
            value: Some(json!({})),
            _reason: None,
        }];

        apply_default_config_overrides(&mut root, &overrides).expect("overrides apply");

        assert_eq!(root, json!({ "sankey": { "nodeColors": {} } }));
    }

    #[test]
    fn default_config_output_sorts_json_keys_recursively() {
        let mut root = json!({
            "z": 1,
            "a": {
                "textPosition": 0.75,
                "donutHole": 0
            },
            "m": [
                {
                    "b": true,
                    "a": false
                }
            ]
        });

        sort_json_value_keys(&mut root);

        let top_keys: Vec<&str> = root
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(top_keys, vec!["a", "m", "z"]);
        let nested_keys: Vec<&str> = root["a"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(nested_keys, vec!["donutHole", "textPosition"]);
        let array_object_keys: Vec<&str> = root["m"][0]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(array_object_keys, vec!["a", "b"]);
    }

    #[test]
    fn dompurify_generated_header_uses_current_baseline_version() {
        let rust = render_dompurify_defaults_rs(&[], &[], &[], &[]);

        assert!(rust.contains(&format!("DOMPurify {DOMPURIFY_BASELINE_VERSION}")));
    }

    #[test]
    fn dompurify_missing_reference_message_is_actionable() {
        let message = super::dompurify_reference_checkout_message(std::path::Path::new(
            "repo-ref/dompurify/dist/purify.cjs.js",
        ));

        assert!(message.contains("repo-ref/dompurify"));
        assert!(message.contains(DOMPURIFY_BASELINE_VERSION));
        assert!(message.contains("tools/upstreams/REPOS.lock.json"));
    }

    #[test]
    fn venn_upstream_svg_tools_include_admitted_diagram() {
        assert!(UPSTREAM_SVG_DIAGRAMS.contains(&"venn"));
    }

    #[test]
    fn mermaid_11_16_new_families_are_available_to_upstream_svg_tools() {
        for diagram in [
            "cynefin",
            "railroad",
            "railroadEbnf",
            "railroadAbnf",
            "railroadPeg",
        ] {
            assert!(
                UPSTREAM_SVG_DIAGRAMS.contains(&diagram),
                "{diagram} should be exportable before primary admission"
            );
        }
    }

    #[test]
    fn upstream_svg_diagram_list_has_no_duplicates() {
        let diagrams = UPSTREAM_SVG_DIAGRAMS
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(diagrams.len(), UPSTREAM_SVG_DIAGRAMS.len());
    }

    #[test]
    fn upstream_svg_all_commands_use_the_same_diagram_set() {
        assert!(UPSTREAM_SVG_DIAGRAMS.contains(&"xychart"));
    }

    #[test]
    fn upstream_svg_jobs_accept_positive_values_and_reject_invalid_values() {
        assert_eq!(parse_upstream_svg_jobs(Some("1")).unwrap().get(), 1);
        assert_eq!(parse_upstream_svg_jobs(Some(" 4 ")).unwrap().get(), 4);
        assert!(matches!(
            parse_upstream_svg_jobs(None),
            Err(crate::XtaskError::Usage)
        ));

        for invalid in ["0", "-1", "invalid"] {
            let error = parse_upstream_svg_jobs(Some(invalid))
                .expect_err("non-positive and non-numeric job counts must fail")
                .to_string();
            assert!(error.contains("--jobs"), "unexpected error: {error}");
            assert!(error.contains("greater than or equal to 1"));
        }
    }

    #[test]
    fn upstream_svg_generation_options_require_explicit_non_flag_values() {
        for option in [
            "--diagram",
            "--out",
            "--filter",
            "--fixtures-root",
            "--jobs",
        ] {
            let missing = vec![option.to_string()];
            assert!(matches!(
                parse_gen_upstream_svgs_options(&missing),
                Err(crate::XtaskError::Usage)
            ));

            let followed_by_flag = vec![option.to_string(), "--fresh-output".to_string()];
            assert!(matches!(
                parse_gen_upstream_svgs_options(&followed_by_flag),
                Err(crate::XtaskError::Usage)
            ));

            let followed_by_short_flag = vec![option.to_string(), "-x".to_string()];
            assert!(matches!(
                parse_gen_upstream_svgs_options(&followed_by_short_flag),
                Err(crate::XtaskError::Usage)
            ));

            let empty = vec![option.to_string(), "   ".to_string()];
            assert!(matches!(
                parse_gen_upstream_svgs_options(&empty),
                Err(crate::XtaskError::Usage)
            ));
        }
        for unsafe_without_out in [
            vec![
                "--fixtures-root".to_string(),
                "scratch-fixtures".to_string(),
            ],
            vec!["--fresh-output".to_string()],
        ] {
            assert!(matches!(
                parse_gen_upstream_svgs_options(&unsafe_without_out),
                Err(crate::XtaskError::Usage)
            ));
        }

        let parsed = parse_gen_upstream_svgs_options(&[
            "--diagram".to_string(),
            "info".to_string(),
            "--out".to_string(),
            "target/upstream".to_string(),
            "--filter".to_string(),
            "fixture_001".to_string(),
            "--fixtures-root".to_string(),
            "fixtures".to_string(),
            "--jobs".to_string(),
            "2".to_string(),
            "--fresh-output".to_string(),
            "--install".to_string(),
        ])
        .expect("parse complete upstream SVG generation options");
        assert_eq!(parsed.diagram, "info");
        assert_eq!(parsed.out_root, Some(PathBuf::from("target/upstream")));
        assert_eq!(parsed.filter.as_deref(), Some("fixture_001"));
        assert_eq!(parsed.fixtures_root, Some(PathBuf::from("fixtures")));
        assert_eq!(parsed.jobs.get(), 2);
        assert!(parsed.fresh_output);
        assert!(parsed.install);
    }

    #[test]
    fn upstream_svg_custom_paths_are_made_absolute_from_the_workspace() {
        let workspace_root = unique_test_root("upstream-svg-absolute-path");
        assert!(workspace_root.is_absolute());

        let relative =
            absolutize_workspace_path(&workspace_root, PathBuf::from("target/custom-upstream"))
                .expect("resolve a workspace-relative custom output");
        assert!(relative.is_absolute());
        assert_eq!(relative, workspace_root.join("target/custom-upstream"));
        assert!(
            relative
                .join(".xtask-upstream-svg-staging/architecture/run-1/inputs")
                .is_absolute(),
            "seeded renderer snapshots must never depend on the Node working directory"
        );

        let absolute = workspace_root.join("already-absolute");
        assert_eq!(
            absolutize_workspace_path(&workspace_root, absolute.clone())
                .expect("preserve an absolute custom output"),
            absolute
        );
        #[cfg(windows)]
        for invalid in ["C:drive-relative", r"\root-relative"] {
            assert!(
                absolutize_workspace_path(&workspace_root, PathBuf::from(invalid)).is_err(),
                "non-absolute Windows root or drive paths must not reach the seeded renderer: {invalid}"
            );
        }
    }

    #[test]
    fn upstream_svg_all_filter_selects_only_matching_families() {
        let fixtures_root = unique_test_root("upstream-svg-global-filter");
        let info_dir = fixtures_root.join("info");
        let sequence_dir = fixtures_root.join("sequence");
        fs::create_dir_all(&info_dir).expect("create info fixtures");
        fs::create_dir_all(&sequence_dir).expect("create sequence fixtures");
        fs::write(info_dir.join("selected_fixture.mmd"), "info\n").expect("write matching fixture");
        fs::write(sequence_dir.join("other_fixture.mmd"), "sequenceDiagram\n")
            .expect("write non-matching fixture");

        assert_eq!(
            select_upstream_svg_diagrams("all", &fixtures_root, Some("selected_fixture"))
                .expect("select the matching family"),
            vec!["info"]
        );
        let missing = select_upstream_svg_diagrams("all", &fixtures_root, Some("missing"))
            .expect_err("a globally unmatched filter must fail before generation");
        assert!(missing.to_string().contains("no .mmd fixtures matched"));

        let single_missing =
            select_upstream_svg_diagrams("sequence", &fixtures_root, Some("selected_fixture"))
                .expect_err("a family-local unmatched filter must fail before generation");
        assert!(single_missing.to_string().contains("sequence"));

        let selected_path = info_dir.join("selected_fixture.mmd");
        let captured_selection = upstream_svg_filter_matches(&info_dir, "selected_fixture");
        fs::rename(&selected_path, info_dir.join("renamed_fixture.mmd"))
            .expect("change the requested selection before snapshot validation");
        let changed = validate_upstream_svg_filter_selection(
            &info_dir,
            "selected_fixture",
            &captured_selection,
        )
        .expect_err("an adopted upgrade must not outlive its original filter match");
        assert!(changed.to_string().contains("selection"));
        remove_test_root(&fixtures_root);
    }

    #[test]
    fn fresh_upstream_svg_output_must_still_be_empty_at_final_preflight() {
        let out_dir = unique_test_root("upstream-svg-fresh-final-preflight");
        fs::create_dir_all(&out_dir).expect("create fresh output directory");
        let family_lock =
            acquire_upstream_svg_family_lock(&out_dir).expect("hold the final family lock");
        ensure_fresh_upstream_svg_output_is_empty(&out_dir, true)
            .expect("the external lock file must not make fresh output non-empty");

        fs::write(out_dir.join("concurrent.svg"), "<svg/>").expect("simulate a concurrent writer");
        let error = ensure_fresh_upstream_svg_output_is_empty(&out_dir, true)
            .expect_err("a later output must invalidate fresh generation");
        assert!(error.to_string().contains("non-empty directory"));
        ensure_fresh_upstream_svg_output_is_empty(&out_dir, false)
            .expect("non-fresh generation does not require an empty directory");
        drop(family_lock);
        remove_test_root(&out_dir);
    }

    #[test]
    fn upstream_svg_failure_reports_do_not_mutate_the_family_output() {
        let root = unique_test_root("upstream-svg-failure-report");
        let out_dir = root.join("upstream").join("sequence");
        let staging_dir = root
            .join("upstream")
            .join(".xtask-upstream-svg-staging")
            .join("sequence");
        fs::create_dir_all(&out_dir).expect("create family output directory");
        fs::create_dir_all(&staging_dir).expect("create sibling staging directory");

        let first = unique_upstream_svg_failure_report_path(&staging_dir);
        let second = unique_upstream_svg_failure_report_path(&staging_dir);
        assert!(first.starts_with(&staging_dir));
        assert!(!first.starts_with(&out_dir));
        assert_ne!(first, second, "concurrent failures need distinct reports");
        fs::write(&first, "render failed").expect("write staged failure report");
        ensure_fresh_upstream_svg_output_is_empty(&out_dir, true)
            .expect("a sibling failure report must not contaminate fresh output");

        remove_test_root(&root);
    }

    #[test]
    fn external_upstream_svg_family_lock_is_validated_and_reused() {
        let root = unique_test_root("upstream-svg-external-family-lock");
        let out_root = root.join("upstream");
        let locked_dir = out_root.join("sequence");
        let other_dir = out_root.join("info");
        fs::create_dir_all(&locked_dir).expect("create locked family directory");
        fs::create_dir_all(&other_dir).expect("create other family directory");
        let held_lock =
            acquire_upstream_svg_family_lock(&locked_dir).expect("acquire external family lock");

        validate_external_upstream_svg_family_lock(
            "sequence",
            &["sequence"],
            &out_root,
            &held_lock,
        )
        .expect("matching external family lock should be accepted");
        let borrowed = use_or_acquire_upstream_svg_family_lock(&locked_dir, Some(&held_lock))
            .expect("an existing lock must be borrowed without reacquiring it");
        borrowed
            .validate_target(&locked_dir)
            .expect("borrowed lock still protects the requested family");
        drop(borrowed);

        let wrong_family =
            validate_external_upstream_svg_family_lock("info", &["info"], &out_root, &held_lock)
                .expect_err("a lock for another family must be rejected");
        assert!(wrong_family.to_string().contains("protects"));
        let all =
            validate_external_upstream_svg_family_lock("all", &["sequence"], &out_root, &held_lock)
                .expect_err("an external family lock cannot authorize an all-family request");
        assert!(all.to_string().contains("one explicit diagram"));

        drop(held_lock);
        remove_test_root(&root);
    }

    #[test]
    fn parser_only_fixtures_use_the_same_partition_for_generation_and_check() {
        let renderable = PathBuf::from("regular.mmd");
        let parser_only = PathBuf::from("syntax_parser_only_spec.mmd");
        let (renderable_fixtures, excluded_fixtures) =
            partition_upstream_svg_fixtures("sequence", [renderable.clone(), parser_only.clone()])
                .expect("partition fixtures");

        assert_eq!(renderable_fixtures, vec![renderable]);
        assert_eq!(excluded_fixtures.len(), 1);
        assert_eq!(excluded_fixtures[0].0, parser_only);
        assert!(excluded_fixtures[0].1.contains("parser-only"));
    }

    #[test]
    fn generated_js_scripts_are_content_addressed_and_installed_atomically() {
        let root = unique_test_root("content-addressed-js");
        let script_dir = root.join("scripts");
        let contents = "process.stdout.write('ready');\n";
        let worker_count = 8usize;
        let barrier = Arc::new(Barrier::new(worker_count));
        let handles: Vec<_> = (0..worker_count)
            .map(|_| {
                let barrier = barrier.clone();
                let script_dir = script_dir.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    ensure_content_addressed_js_script(&script_dir, "probe", contents)
                        .map_err(|err| err.to_string())
                })
            })
            .collect();
        let paths: Vec<_> = handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .expect("script writer thread")
                    .expect("install content-addressed script")
            })
            .collect();

        assert!(paths.iter().all(|path| path == &paths[0]));
        assert_eq!(
            fs::read_to_string(&paths[0]).expect("read installed script"),
            contents
        );
        assert_eq!(
            fs::read_dir(&script_dir)
                .expect("read script directory")
                .count(),
            1,
            "concurrent writers must not leave staging files"
        );
        remove_test_root(&root);
    }

    #[test]
    fn missing_or_invalid_temporary_svg_never_reuses_the_existing_output() {
        let root = unique_test_root("upstream-svg-temp-reuse");
        fs::create_dir_all(&root).expect("create test root");
        let out_path = root.join("baseline.svg");
        fs::write(&out_path, r#"<svg id="old"/>"#).expect("write existing baseline");

        let missing_temp = unique_test_svg_temp_path(&root, &out_path);
        let missing_error = validate_and_promote_upstream_svg_temp(&missing_temp, &out_path)
            .expect_err("missing temporary output must fail");
        assert!(missing_error.contains("did not produce temporary SVG"));
        assert_eq!(
            fs::read_to_string(&out_path).expect("read existing baseline"),
            r#"<svg id="old"/>"#
        );

        let invalid_temp = unique_test_svg_temp_path(&root, &out_path);
        fs::write(&invalid_temp, "not svg").expect("write invalid temporary output");
        let invalid_error = validate_and_promote_upstream_svg_temp(&invalid_temp, &out_path)
            .expect_err("non-SVG temporary output must fail");
        assert!(invalid_error.contains("not an SVG document"));
        assert!(!invalid_temp.exists());
        assert_eq!(
            fs::read_to_string(&out_path).expect("read existing baseline"),
            r#"<svg id="old"/>"#
        );

        remove_test_root(&root);
    }

    #[test]
    fn validated_temporary_svg_replaces_the_existing_output_and_is_cleaned() {
        let root = unique_test_root("upstream-svg-temp-promote");
        fs::create_dir_all(&root).expect("create test root");
        let out_path = root.join("baseline.svg");
        fs::write(&out_path, r#"<svg id="old"/>"#).expect("write existing baseline");
        let temp_path = unique_test_svg_temp_path(&root, &out_path);
        fs::write(&temp_path, r#"<svg id="new"><g/></svg>"#).expect("write temporary SVG");

        validate_and_promote_upstream_svg_temp(&temp_path, &out_path)
            .expect("valid temporary SVG is promoted");

        assert_eq!(
            fs::read_to_string(&out_path).expect("read promoted baseline"),
            r#"<svg id="new"><g/></svg>"#
        );
        assert!(!temp_path.exists());
        remove_test_root(&root);
    }

    #[test]
    fn upstream_svg_batch_deletes_excluded_output_after_metadata_commit() {
        let root = unique_test_root("upstream-svg-batch-delete");
        fs::create_dir_all(&root).expect("create test root");
        let deleted_out = root.join("excluded.svg");
        fs::write(&deleted_out, r#"<svg id="old"/>"#).expect("write excluded baseline");

        promote_upstream_svg_batch(&[], std::slice::from_ref(&deleted_out), || Ok(()))
            .expect("committed deletion should succeed");

        assert!(!deleted_out.exists());
        assert!(fs::read_dir(&root).expect("read test root").all(|entry| {
            !entry
                .expect("read directory entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".backup")
        }));
        remove_test_root(&root);
    }

    #[test]
    fn temporary_svg_batch_is_all_or_nothing_when_any_output_is_invalid() {
        let root = unique_test_root("upstream-svg-batch-validation");
        fs::create_dir_all(&root).expect("create test root");
        let first_out = root.join("first.svg");
        let second_out = root.join("second.svg");
        fs::write(&first_out, r#"<svg id="first-old"/>"#).expect("write first baseline");
        fs::write(&second_out, r#"<svg id="second-old"/>"#).expect("write second baseline");

        let first_temp = unique_test_svg_temp_path(&root, &first_out);
        let second_temp = unique_test_svg_temp_path(&root, &second_out);
        fs::write(&first_temp, r#"<svg id="first-new"/>"#).expect("write first temp");
        fs::write(&second_temp, "not svg").expect("write invalid second temp");
        let pending = [
            PendingUpstreamSvg {
                temp_path: first_temp.clone(),
                out_path: first_out.clone(),
            },
            PendingUpstreamSvg {
                temp_path: second_temp.clone(),
                out_path: second_out.clone(),
            },
        ];

        let error = promote_upstream_svg_batch(&pending, &[], || Ok(()))
            .expect_err("one invalid output rejects the whole batch");

        assert!(error.contains("not an SVG document"), "{error}");
        assert_eq!(
            fs::read_to_string(&first_out).expect("read first baseline"),
            r#"<svg id="first-old"/>"#
        );
        assert_eq!(
            fs::read_to_string(&second_out).expect("read second baseline"),
            r#"<svg id="second-old"/>"#
        );
        assert!(!first_temp.exists());
        assert!(!second_temp.exists());
        remove_test_root(&root);
    }

    #[test]
    fn temporary_svg_batch_rolls_back_when_metadata_commit_fails() {
        let root = unique_test_root("upstream-svg-batch-metadata");
        fs::create_dir_all(&root).expect("create test root");
        let first_out = root.join("first.svg");
        let second_out = root.join("second.svg");
        let deleted_out = root.join("excluded.svg");
        fs::write(&first_out, r#"<svg id="first-old"/>"#).expect("write first baseline");
        fs::write(&second_out, r#"<svg id="second-old"/>"#).expect("write second baseline");
        fs::write(&deleted_out, r#"<svg id="excluded-old"/>"#).expect("write excluded baseline");

        let first_temp = unique_test_svg_temp_path(&root, &first_out);
        let second_temp = unique_test_svg_temp_path(&root, &second_out);
        fs::write(&first_temp, r#"<svg id="first-new"/>"#).expect("write first temp");
        fs::write(&second_temp, r#"<svg id="second-new"/>"#).expect("write second temp");
        let pending = [
            PendingUpstreamSvg {
                temp_path: first_temp.clone(),
                out_path: first_out.clone(),
            },
            PendingUpstreamSvg {
                temp_path: second_temp.clone(),
                out_path: second_out.clone(),
            },
        ];

        let error =
            promote_upstream_svg_batch(&pending, std::slice::from_ref(&deleted_out), || {
                assert!(
                    !deleted_out.exists(),
                    "deletion must precede metadata commit"
                );
                Err("metadata commit rejected".to_string())
            })
            .expect_err("metadata failure rolls back SVG promotion");

        assert!(error.contains("metadata commit rejected"), "{error}");
        assert_eq!(
            fs::read_to_string(&first_out).expect("read first baseline"),
            r#"<svg id="first-old"/>"#
        );
        assert_eq!(
            fs::read_to_string(&second_out).expect("read second baseline"),
            r#"<svg id="second-old"/>"#
        );
        assert_eq!(
            fs::read_to_string(&deleted_out).expect("read restored excluded baseline"),
            r#"<svg id="excluded-old"/>"#
        );
        assert!(!first_temp.exists());
        assert!(!second_temp.exists());
        assert!(fs::read_dir(&root).expect("read test root").all(|entry| {
            !entry
                .expect("read directory entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".backup")
        }));
        remove_test_root(&root);
    }

    #[test]
    fn temporary_svg_batch_rolls_back_when_promotion_fails_mid_batch() {
        let root = unique_test_root("upstream-svg-batch-promotion");
        fs::create_dir_all(&root).expect("create test root");
        let first_out = root.join("first.svg");
        let second_out = root.join("second.svg");
        let deleted_out = root.join("excluded.svg");
        fs::write(&first_out, r#"<svg id="first-old"/>"#).expect("write first baseline");
        fs::write(&second_out, r#"<svg id="second-old"/>"#).expect("write second baseline");
        fs::write(&deleted_out, r#"<svg id="excluded-old"/>"#).expect("write excluded baseline");

        let shared_temp = unique_test_svg_temp_path(&root, &first_out);
        fs::write(&shared_temp, r#"<svg id="new"/>"#).expect("write shared temp");
        let pending = [
            PendingUpstreamSvg {
                temp_path: shared_temp.clone(),
                out_path: first_out.clone(),
            },
            PendingUpstreamSvg {
                temp_path: shared_temp.clone(),
                out_path: second_out.clone(),
            },
        ];
        let metadata_committed = std::cell::Cell::new(false);

        let error =
            promote_upstream_svg_batch(&pending, std::slice::from_ref(&deleted_out), || {
                metadata_committed.set(true);
                Ok(())
            })
            .expect_err("the reused temp path must fail on the second promotion");

        assert!(
            error.contains("failed to promote temporary upstream SVG"),
            "{error}"
        );
        assert!(!metadata_committed.get());
        assert_eq!(
            fs::read_to_string(&first_out).expect("read first baseline"),
            r#"<svg id="first-old"/>"#
        );
        assert_eq!(
            fs::read_to_string(&second_out).expect("read second baseline"),
            r#"<svg id="second-old"/>"#
        );
        assert_eq!(
            fs::read_to_string(&deleted_out).expect("read restored excluded baseline"),
            r#"<svg id="excluded-old"/>"#
        );
        assert!(!shared_temp.exists());
        assert!(fs::read_dir(&root).expect("read test root").all(|entry| {
            !entry
                .expect("read directory entry")
                .file_name()
                .to_string_lossy()
                .ends_with(".backup")
        }));
        remove_test_root(&root);
    }

    #[test]
    fn upstream_svg_timeout_child_helper() {
        if std::env::var_os("MERMAN_XTASK_TIMEOUT_CHILD").is_some() {
            std::thread::sleep(Duration::from_secs(30));
        }
    }

    #[test]
    fn upstream_svg_large_pipe_child_helper() {
        if std::env::var_os("MERMAN_XTASK_LARGE_PIPE_CHILD").is_none() {
            return;
        }

        const PAYLOAD_BYTES: usize = 512 * 1024;
        let stdout_writer = std::thread::spawn(|| {
            let bytes = vec![b'o'; PAYLOAD_BYTES];
            std::io::stdout()
                .lock()
                .write_all(&bytes)
                .expect("write large stdout payload");
        });
        let stderr_writer = std::thread::spawn(|| {
            let bytes = vec![b'e'; PAYLOAD_BYTES];
            std::io::stderr()
                .lock()
                .write_all(&bytes)
                .expect("write large stderr payload");
        });
        stdout_writer.join().expect("join stdout writer");
        stderr_writer.join().expect("join stderr writer");
    }

    #[test]
    fn upstream_svg_process_tree_grandchild_helper() {
        let Some(ready_path) = std::env::var_os("MERMAN_XTASK_TREE_GRANDCHILD_READY") else {
            return;
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind process-tree grandchild listener");
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
    fn upstream_svg_process_tree_child_helper() {
        let Some(ready_path) = std::env::var_os("MERMAN_XTASK_TREE_CHILD_READY") else {
            return;
        };
        let executable = std::env::current_exe().expect("current test executable");
        let test_name = format!(
            "{}::upstream_svg_process_tree_grandchild_helper",
            module_path!()
        );
        let test_name = test_name
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(test_name.as_str());
        let mut grandchild = Command::new(executable)
            .args(["--exact", test_name, "--nocapture"])
            .env("MERMAN_XTASK_TREE_GRANDCHILD_READY", ready_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn process-tree grandchild");
        std::thread::sleep(Duration::from_secs(30));
        let _ = grandchild.wait();
    }

    #[test]
    fn upstream_svg_timeout_terminates_the_managed_process_tree() {
        let root = unique_test_root("upstream-svg-process-tree");
        fs::create_dir_all(&root).expect("create process-tree test root");
        let ready_path = root.join("grandchild-ready.txt");
        let executable = std::env::current_exe().expect("current test executable");
        let test_name = format!("{}::upstream_svg_process_tree_child_helper", module_path!());
        let test_name = test_name
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(test_name.as_str());
        let mut command = Command::new(executable);
        command
            .args(["--exact", test_name, "--nocapture"])
            .env("MERMAN_XTASK_TREE_CHILD_READY", &ready_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child =
            spawn_timeout_managed_child(&mut command).expect("spawn process-tree child");

        let ready_deadline = Instant::now() + Duration::from_secs(5);
        while !ready_path.is_file() && Instant::now() < ready_deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let address: std::net::SocketAddr = fs::read_to_string(&ready_path)
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
                .is_some()
        );

        let release_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match std::net::TcpListener::bind(address) {
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
        remove_test_root(&root);
    }

    #[test]
    fn upstream_svg_process_wait_enforces_a_hard_timeout() {
        let executable = std::env::current_exe().expect("current test executable");
        let test_name = format!("{}::upstream_svg_timeout_child_helper", module_path!());
        let test_name = test_name
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(test_name.as_str());
        let mut command = Command::new(executable);
        command
            .args(["--exact", test_name, "--nocapture"])
            .env("MERMAN_XTASK_TIMEOUT_CHILD", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = spawn_timeout_managed_child(&mut command).expect("spawn timeout child");
        let started = Instant::now();

        let error = wait_with_timeout(&mut child, Duration::from_millis(100))
            .expect_err("sleeping child must time out");

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(
            child.try_wait().expect("query terminated child").is_some(),
            "timed-out child must be reaped before wait_with_timeout returns"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "hard timeout should terminate promptly"
        );
    }

    #[test]
    fn upstream_svg_probe_drains_large_stdout_and_stderr_without_backpressure() {
        let executable = std::env::current_exe().expect("current test executable");
        let test_name = format!("{}::upstream_svg_large_pipe_child_helper", module_path!());
        let test_name = test_name
            .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
            .unwrap_or(test_name.as_str());
        let mut command = Command::new(executable);
        command
            .args(["--exact", test_name, "--nocapture"])
            .env("MERMAN_XTASK_LARGE_PIPE_CHILD", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = spawn_timeout_managed_child(&mut command).expect("spawn large-pipe child");
        let started = Instant::now();

        let output = wait_with_bounded_output(&mut child, Duration::from_secs(5), 1024 * 1024)
            .expect("large probe output should be drained concurrently");

        assert!(output.status.success());
        assert!(
            output.stdout.iter().filter(|byte| **byte == b'o').count() >= 512 * 1024,
            "stdout payload was truncated"
        );
        assert!(
            output.stderr.iter().filter(|byte| **byte == b'e').count() >= 512 * 1024,
            "stderr payload was truncated"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn bounded_probe_reader_drains_to_eof_after_reaching_its_limit() {
        let bytes = vec![b'x'; 4096];
        let mut cursor = Cursor::new(bytes);

        let error = read_bounded_child_pipe(&mut cursor, "test", 1024)
            .expect_err("oversized output must be rejected");

        assert!(error.to_string().contains("exceeded 1024 bytes"));
        assert_eq!(cursor.position(), 4096);
    }

    #[test]
    fn upstream_svg_family_lock_serializes_writers() {
        let root = unique_test_root("upstream-svg-family-lock");
        fs::create_dir_all(&root).expect("create lock output directory");
        let first = acquire_upstream_svg_family_lock(&root).expect("acquire first family lock");

        let blocked =
            acquire_upstream_svg_family_lock_with_timeout(&root, Duration::from_millis(50))
                .expect_err("a second writer must not enter the same family transaction");
        assert!(
            blocked.to_string().contains("timed out waiting"),
            "{blocked}"
        );

        drop(first);
        acquire_upstream_svg_family_lock_with_timeout(&root, Duration::from_secs(1))
            .expect("released family lock should be reusable");
        remove_test_root(&root);
    }

    #[test]
    fn bounded_fixture_jobs_preserve_failure_order_and_limit_concurrency() {
        let fixtures: Vec<usize> = (0..12).collect();
        let active = AtomicUsize::new(0);
        let max_active = AtomicUsize::new(0);
        let jobs = std::num::NonZeroUsize::new(3).expect("non-zero jobs");

        let results = map_bounded_in_order(&fixtures, jobs, |fixture| {
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            max_active.fetch_max(current, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(
                ((12 - *fixture) % 4 + 1) as u64 * 3,
            ));
            active.fetch_sub(1, Ordering::SeqCst);
            fixture
                .is_multiple_of(2)
                .then(|| format!("fixture-{fixture} failed"))
        });
        let failures: Vec<_> = results.into_iter().flatten().collect();

        assert_eq!(
            failures,
            [
                "fixture-0 failed",
                "fixture-2 failed",
                "fixture-4 failed",
                "fixture-6 failed",
                "fixture-8 failed",
                "fixture-10 failed",
            ]
        );
        assert!(max_active.load(Ordering::SeqCst) <= jobs.get());
        assert!(
            max_active.load(Ordering::SeqCst) >= 2,
            "the test should exercise concurrent workers"
        );
    }

    #[test]
    fn upstream_svg_check_output_is_unique_and_starts_empty() {
        let target_root = unique_test_root("upstream-svg-check-output");
        let first = create_upstream_svg_check_output_root(&target_root).expect("first check root");
        fs::write(first.join("stale.svg"), "stale").expect("write stale marker");

        let second =
            create_upstream_svg_check_output_root(&target_root).expect("second check root");

        assert_ne!(first, second);
        assert!(second.is_dir());
        assert!(
            fs::read_dir(&second)
                .expect("read second check root")
                .next()
                .is_none(),
            "a new upstream SVG check must not see artifacts from an earlier run"
        );
        remove_test_root(&target_root);
    }

    #[test]
    fn requirement_font_precedence_fixture_always_uses_strict_fresh_render_check() {
        for (check_dom, requested_mode) in [(false, DomMode::Structure), (true, DomMode::Parity)] {
            assert_eq!(
                upstream_svg_check_dom_mode(
                    "requirement",
                    REQUIREMENT_FONT_PRECEDENCE_FIXTURE,
                    check_dom,
                    requested_mode,
                ),
                Some(DomMode::Strict)
            );
        }

        assert_eq!(
            upstream_svg_check_dom_mode("requirement", "basic", false, DomMode::Strict,),
            Some(DomMode::Structure)
        );
        assert_eq!(
            upstream_svg_check_dom_mode("sequence", "basic", false, DomMode::Strict,),
            None
        );
    }

    #[test]
    fn mermaid_cli_install_validation_rejects_stale_mermaid_and_cli_versions() {
        let tools_root = unique_test_root("mermaid-cli-install");
        write_mermaid_cli_install_fixture(&tools_root, "11.16.0", "11.15.0");

        let stale_mermaid =
            validate_mermaid_cli_install(&tools_root).expect_err("stale Mermaid must fail");
        let stale_mermaid = stale_mermaid.to_string();
        assert!(stale_mermaid.contains("mermaid"));
        assert!(stale_mermaid.contains("11.15.0"));
        assert!(stale_mermaid.contains("11.16.0"));

        write_mermaid_cli_install_fixture(&tools_root, "11.15.0", "11.16.0");
        let stale_cli = validate_mermaid_cli_install(&tools_root).expect_err("stale CLI must fail");
        let stale_cli = stale_cli.to_string();
        assert!(stale_cli.contains("@mermaid-js/mermaid-cli"));
        assert!(stale_cli.contains("11.15.0"));
        assert!(stale_cli.contains("11.16.0"));

        write_mermaid_cli_install_fixture(&tools_root, "11.16.0", "11.16.0");
        let entry =
            validate_mermaid_cli_install(&tools_root).expect("matching install should pass");
        assert_eq!(
            entry,
            tools_root.join("node_modules/@mermaid-js/mermaid-cli/src/cli.js")
        );
        #[cfg(windows)]
        assert!(
            !entry.to_string_lossy().starts_with(r"\\?\"),
            "Node entry points must not use a Windows verbatim path: {}",
            entry.display()
        );
        assert!(
            !tools_root.join("node_modules/.bin").exists(),
            "validation must not depend on an npm-generated shim"
        );
        remove_test_root(&tools_root);
    }

    #[test]
    fn mermaid_cli_install_validation_rejects_an_entry_outside_the_package_tree() {
        let tools_root = unique_test_root("mermaid-cli-entry-containment");
        write_mermaid_cli_install_fixture(&tools_root, "11.16.0", "11.16.0");
        let outside_entry = tools_root.join("outside.js");
        fs::write(&outside_entry, "#!/usr/bin/env node\n").expect("write outside entry");
        let manifest_path = tools_root.join("node_modules/@mermaid-js/mermaid-cli/package.json");
        fs::write(
            &manifest_path,
            serde_json::to_vec(&json!({
                "name": "@mermaid-js/mermaid-cli",
                "version": "11.16.0",
                "bin": { "mmdc": "../../../outside.js" }
            }))
            .expect("serialize escaping CLI package manifest"),
        )
        .expect("write escaping CLI package manifest");

        let error = validate_mermaid_cli_install(&tools_root)
            .expect_err("an entry outside the fingerprinted package must fail");

        assert!(error.to_string().contains("inside"), "{error}");
        remove_test_root(&tools_root);
    }

    #[test]
    fn render_probe_requires_real_browser_and_matching_runtime_versions() {
        let test_root = unique_test_root("render-probe-validation");
        fs::create_dir_all(&test_root).expect("create render probe test root");
        let browser_executable = test_root.join("chrome.exe");
        fs::write(&browser_executable, b"test browser").expect("write browser executable");
        let mermaid_root = test_root.join("mermaid");
        let mermaid_cli_root = test_root.join("mermaid-cli");
        write_package_manifest(&mermaid_root.join("package.json"), "mermaid", "11.16.0");
        write_package_manifest(
            &mermaid_cli_root.join("package.json"),
            "@mermaid-js/mermaid-cli",
            "11.16.0",
        );
        let runtime_package_roots = UpstreamSvgRuntimePackageRoots {
            mermaid: mermaid_root,
            mermaid_cli: mermaid_cli_root,
        };

        let environment = crate::cmd::UpstreamSvgRenderEnvironment {
            browser: crate::cmd::UpstreamSvgBrowserEnvironment {
                product: "Chrome".to_string(),
                version: "131.0.6778.204".to_string(),
                revision: "@revision".to_string(),
            },
            puppeteer: crate::cmd::UpstreamSvgPuppeteerEnvironment {
                version: "23.11.1".to_string(),
            },
            operating_system: crate::cmd::UpstreamSvgOperatingSystemEnvironment {
                platform: "win32".to_string(),
                arch: "x64".to_string(),
                release: "test".to_string(),
            },
            mermaid_runtime: crate::cmd::UpstreamSvgRuntimeEnvironment {
                esm_version: "11.16.0".to_string(),
                iife_version: "11.16.0".to_string(),
                mermaid_package_sha256: PINNED_MERMAID_PACKAGE_SHA256.to_string(),
                mermaid_cli_package_sha256: PINNED_MERMAID_CLI_PACKAGE_SHA256.to_string(),
            },
            font_probe: crate::cmd::UpstreamSvgFontProbeEnvironment {
                revision: "mermaid-font-probe-v1".to_string(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            },
        };

        validate_upstream_svg_render_probe(
            UpstreamSvgRenderProbe {
                render_environment: environment.clone(),
                browser_executable: browser_executable.clone(),
                runtime_package_roots: runtime_package_roots.clone(),
            },
            "11.16.0",
        )
        .expect("matching runtime versions and a real executable are valid");

        let mut stale_environment = environment.clone();
        stale_environment.mermaid_runtime.iife_version = "11.15.0".to_string();
        let stale = validate_upstream_svg_render_probe(
            UpstreamSvgRenderProbe {
                render_environment: stale_environment,
                browser_executable: browser_executable.clone(),
                runtime_package_roots: runtime_package_roots.clone(),
            },
            "11.16.0",
        )
        .expect_err("a stale IIFE runtime must fail");
        assert!(stale.to_string().contains("IIFE=11.15.0"));

        let mut modified_runtime = environment.clone();
        modified_runtime.mermaid_runtime.mermaid_package_sha256 =
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
        let modified = validate_upstream_svg_render_probe(
            UpstreamSvgRenderProbe {
                render_environment: modified_runtime,
                browser_executable: browser_executable.clone(),
                runtime_package_roots: runtime_package_roots.clone(),
            },
            "11.16.0",
        )
        .expect_err("same-version modified Mermaid runtime content must fail");
        assert!(modified.to_string().contains("package content"));

        fs::remove_file(&browser_executable).expect("remove browser executable");
        let missing = validate_upstream_svg_render_probe(
            UpstreamSvgRenderProbe {
                render_environment: environment,
                browser_executable,
                runtime_package_roots,
            },
            "11.16.0",
        )
        .expect_err("a missing browser executable must fail");
        assert!(missing.to_string().contains("invalid browser executable"));
        remove_test_root(&test_root);
    }

    #[test]
    fn runtime_package_drift_after_probe_rejects_the_attestation() {
        let test_root = unique_test_root("render-probe-runtime-drift");
        let mermaid_root = test_root.join("mermaid");
        let mermaid_cli_root = test_root.join("mermaid-cli");
        write_package_manifest(&mermaid_root.join("package.json"), "mermaid", "11.16.0");
        write_package_manifest(
            &mermaid_cli_root.join("package.json"),
            "@mermaid-js/mermaid-cli",
            "11.16.0",
        );
        fs::write(mermaid_root.join("runtime.js"), b"original runtime")
            .expect("write Mermaid runtime");
        fs::write(mermaid_cli_root.join("cli.js"), b"original CLI")
            .expect("write Mermaid CLI runtime");
        let mermaid_sha256 =
            upstream_svg_package_tree_sha256(&mermaid_root).expect("hash Mermaid package");
        let mermaid_cli_sha256 =
            upstream_svg_package_tree_sha256(&mermaid_cli_root).expect("hash Mermaid CLI package");
        let environment = crate::cmd::UpstreamSvgRenderEnvironment {
            browser: crate::cmd::UpstreamSvgBrowserEnvironment {
                product: "Chrome".to_string(),
                version: "131.0.6778.204".to_string(),
                revision: "@revision".to_string(),
            },
            puppeteer: crate::cmd::UpstreamSvgPuppeteerEnvironment {
                version: "23.11.1".to_string(),
            },
            operating_system: crate::cmd::UpstreamSvgOperatingSystemEnvironment {
                platform: "win32".to_string(),
                arch: "x64".to_string(),
                release: "test".to_string(),
            },
            mermaid_runtime: crate::cmd::UpstreamSvgRuntimeEnvironment {
                esm_version: "11.16.0".to_string(),
                iife_version: "11.16.0".to_string(),
                mermaid_package_sha256: mermaid_sha256,
                mermaid_cli_package_sha256: mermaid_cli_sha256,
            },
            font_probe: crate::cmd::UpstreamSvgFontProbeEnvironment {
                revision: "mermaid-font-probe-v1".to_string(),
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_string(),
            },
        };
        let probe = UpstreamSvgRenderProbe {
            render_environment: environment.clone(),
            browser_executable: PathBuf::new(),
            runtime_package_roots: UpstreamSvgRuntimePackageRoots {
                mermaid: mermaid_root.clone(),
                mermaid_cli: mermaid_cli_root,
            },
        };
        assert_eq!(
            probe
                .verified_render_environment()
                .expect("unchanged package trees are valid"),
            environment
        );

        fs::write(mermaid_root.join("runtime.js"), b"modified runtime")
            .expect("mutate Mermaid runtime after probe");
        let error = probe
            .verified_render_environment()
            .expect_err("runtime drift must reject the attestation");

        assert!(error.to_string().contains("changed after"), "{error}");
        assert!(error.to_string().contains("mermaid"), "{error}");
        remove_test_root(&test_root);
    }
}
