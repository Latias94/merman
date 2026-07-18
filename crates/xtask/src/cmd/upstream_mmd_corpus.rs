use crate::XtaskError;
use crate::util::sha256_hex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST_FILE_NAME: &str = "_manifest.json";
static CORPUS_TRANSACTION_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct Options {
    from_ref: String,
    to_ref: String,
    corpus_name: String,
    check: bool,
}

#[derive(Debug)]
struct SourceFile {
    path: String,
    contents: Vec<u8>,
    sha256: String,
}

#[derive(Debug)]
struct CorpusPlan {
    manifest: CorpusManifest,
    managed_files: BTreeMap<PathBuf, Vec<u8>>,
}

#[derive(Debug)]
struct CorpusLock {
    file: fs::File,
}

impl Drop for CorpusLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

#[derive(Debug)]
struct PreparedCorpus<'a> {
    manifest: Vec<u8>,
    files: Vec<(PathBuf, &'a [u8])>,
}

#[derive(Debug)]
struct StagedCorpus {
    staging_path: PathBuf,
    backup_path: PathBuf,
}

#[derive(Debug, Deserialize, Serialize)]
struct CorpusManifest {
    schema_version: u32,
    source: CorpusSource,
    summary: CorpusSummary,
    entries: BTreeMap<String, CorpusEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CorpusSource {
    repository: String,
    from_ref: String,
    to_ref: String,
    to_commit: String,
    selection: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct CorpusSummary {
    source_file_count: usize,
    unique_content_count: usize,
    managed_file_count: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct CorpusEntry {
    sha256: String,
    fixture: String,
}

pub(crate) fn sync_upstream_mmd_corpus(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let mermaid_root = crate::cmd::mermaid_repo_root();
    if !mermaid_root.is_dir() {
        return Err(XtaskError::MissingReference(
            mermaid_root.display().to_string(),
        ));
    }

    let source_paths = added_mmd_paths(&mermaid_root, &options)?;
    if source_paths.is_empty() {
        return Err(XtaskError::VerifyFailed(format!(
            "no added .mmd files found between {} and {}",
            options.from_ref, options.to_ref
        )));
    }

    let to_commit = git_text(
        &mermaid_root,
        &["rev-parse", &format!("{}^{{commit}}", options.to_ref)],
    )?;
    let mut sources = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let contents = git_bytes(
            &mermaid_root,
            &["show", &format!("{}:{path}", options.to_ref)],
        )?;
        sources.push(SourceFile {
            path,
            sha256: sha256_hex(&contents),
            contents,
        });
    }

    let fixtures_root = crate::cmd::fixtures_root();
    let corpus_relative_root = PathBuf::from("_upstream").join(&options.corpus_name);
    let corpus_root = fixtures_root.join(&corpus_relative_root);
    let plan = build_plan(sources, &corpus_relative_root, &options, to_commit);

    if options.check {
        check_plan(&fixtures_root, &corpus_root, &plan)?;
        println!(
            "upstream MMD corpus is current: {} source files, {} unique contents, {} managed files",
            plan.manifest.summary.source_file_count,
            plan.manifest.summary.unique_content_count,
            plan.manifest.summary.managed_file_count
        );
        return Ok(());
    }

    write_plan(&fixtures_root, &corpus_root, &plan)?;
    println!(
        "synced upstream MMD corpus: {} source files, {} unique contents, {} managed files",
        plan.manifest.summary.source_file_count,
        plan.manifest.summary.unique_content_count,
        plan.manifest.summary.managed_file_count
    );
    Ok(())
}

fn parse_options(args: Vec<String>) -> Result<Options, XtaskError> {
    let mut from_ref = None;
    let mut to_ref = None;
    let mut corpus_name = None;
    let mut check = false;
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--from" => from_ref = args.next(),
            "--to" => to_ref = args.next(),
            "--name" => corpus_name = args.next(),
            "--check" => check = true,
            "--help" | "-h" => {
                print_usage();
                return Err(XtaskError::Usage);
            }
            _ => return Err(XtaskError::Usage),
        }
    }

    let from_ref = from_ref.ok_or(XtaskError::Usage)?;
    let to_ref = to_ref.ok_or(XtaskError::Usage)?;
    let corpus_name = corpus_name.unwrap_or_else(|| default_corpus_name(&to_ref));
    if corpus_name.is_empty()
        || !corpus_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(XtaskError::VerifyFailed(format!(
            "invalid corpus name {corpus_name:?}; use ASCII letters, digits, '.', '-' or '_'"
        )));
    }

    Ok(Options {
        from_ref,
        to_ref,
        corpus_name,
        check,
    })
}

fn print_usage() {
    println!(
        "usage: xtask sync-upstream-mmd-corpus --from <git-ref> --to <git-ref> [--name <directory>] [--check]"
    );
}

fn default_corpus_name(to_ref: &str) -> String {
    let label = to_ref.strip_prefix("mermaid@").unwrap_or(to_ref);
    let sanitized: String = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    format!("mermaid-{sanitized}")
}

fn added_mmd_paths(mermaid_root: &Path, options: &Options) -> Result<Vec<String>, XtaskError> {
    let range = format!("{}..{}", options.from_ref, options.to_ref);
    let bytes = git_bytes(
        mermaid_root,
        &[
            "diff",
            "--name-only",
            "--diff-filter=A",
            "-z",
            &range,
            "--",
            "*.mmd",
        ],
    )?;
    let mut paths = Vec::new();
    for raw_path in bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = std::str::from_utf8(raw_path).map_err(|error| {
            XtaskError::VerifyFailed(format!("upstream .mmd path is not UTF-8: {error}"))
        })?;
        validate_source_path(path)?;
        paths.push(path.to_string());
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn validate_source_path(path: &str) -> Result<(), XtaskError> {
    let path = Path::new(path);
    if path.extension() != Some(OsStr::new("mmd"))
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err(XtaskError::VerifyFailed(format!(
            "unsafe upstream .mmd path: {}",
            path.display()
        )));
    }
    Ok(())
}

fn git_text(root: &Path, args: &[&str]) -> Result<String, XtaskError> {
    let bytes = git_bytes(root, args)?;
    String::from_utf8(bytes)
        .map(|text| text.trim().to_string())
        .map_err(|error| XtaskError::VerifyFailed(format!("git output is not UTF-8: {error}")))
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, XtaskError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|source| XtaskError::ReadFile {
            path: root.display().to_string(),
            source,
        })?;
    if output.status.success() {
        return Ok(output.stdout);
    }

    Err(XtaskError::VerifyFailed(format!(
        "git {} failed with {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

fn build_plan(
    sources: Vec<SourceFile>,
    corpus_relative_root: &Path,
    options: &Options,
    to_commit: String,
) -> CorpusPlan {
    let source_file_count = sources.len();
    let unique_content_count = sources
        .iter()
        .map(|source| source.sha256.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let mut managed_files = BTreeMap::new();
    let mut entries = BTreeMap::new();

    for source in sources {
        let relative = corpus_relative_root.join("sources").join(&source.path);
        let fixture = slash_path(&relative);
        managed_files.insert(relative, source.contents);
        entries.insert(
            source.path,
            CorpusEntry {
                sha256: source.sha256,
                fixture,
            },
        );
    }

    let managed_file_count = managed_files.len();
    CorpusPlan {
        manifest: CorpusManifest {
            schema_version: 1,
            source: CorpusSource {
                repository: "repo-ref/mermaid".to_string(),
                from_ref: options.from_ref.clone(),
                to_ref: options.to_ref.clone(),
                to_commit,
                selection: "files added between refs with pathspec *.mmd".to_string(),
            },
            summary: CorpusSummary {
                source_file_count,
                unique_content_count,
                managed_file_count,
            },
            entries,
        },
        managed_files,
    }
}

fn write_plan(
    fixtures_root: &Path,
    corpus_root: &Path,
    plan: &CorpusPlan,
) -> Result<(), XtaskError> {
    write_plan_with_renamer(fixtures_root, corpus_root, plan, |from, to| {
        fs::rename(from, to)
    })
}

fn write_plan_with_renamer<R>(
    fixtures_root: &Path,
    corpus_root: &Path,
    plan: &CorpusPlan,
    rename: R,
) -> Result<(), XtaskError>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
{
    let _lock = acquire_corpus_lock(fixtures_root, corpus_root)?;
    let prepared = preflight_plan(fixtures_root, corpus_root, plan)?;
    let staged = stage_corpus(corpus_root, &prepared)?;
    install_staged_corpus(corpus_root, &staged, rename)
}

fn acquire_corpus_lock(fixtures_root: &Path, corpus_root: &Path) -> Result<CorpusLock, XtaskError> {
    let canonical_fixtures_root =
        fs::canonicalize(fixtures_root).map_err(|source| XtaskError::ReadFile {
            path: fixtures_root.display().to_string(),
            source,
        })?;
    let corpus_relative = corpus_root.strip_prefix(fixtures_root).map_err(|_| {
        XtaskError::VerifyFailed(format!(
            "corpus root {} is outside fixtures root {}",
            corpus_root.display(),
            fixtures_root.display()
        ))
    })?;
    if !is_safe_relative_path(corpus_relative) {
        return Err(XtaskError::VerifyFailed(format!(
            "unsafe corpus root {}",
            corpus_root.display()
        )));
    }

    let canonical_target = canonical_fixtures_root.join(corpus_relative);
    let lock_root = std::env::temp_dir()
        .join("merman-xtask-locks")
        .join("upstream-mmd-corpus");
    fs::create_dir_all(&lock_root).map_err(|source| XtaskError::WriteFile {
        path: lock_root.display().to_string(),
        source,
    })?;
    let lock_path = lock_root.join(format!(
        "{}.lock",
        sha256_hex(canonical_target.as_os_str().as_encoded_bytes())
    ));
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
    fs2::FileExt::lock_exclusive(&file).map_err(|source| XtaskError::WriteFile {
        path: lock_path.display().to_string(),
        source,
    })?;
    Ok(CorpusLock { file })
}

fn is_safe_relative_path(path: &Path) -> bool {
    let mut components = path.components();
    components
        .next()
        .is_some_and(|component| matches!(component, Component::Normal(_)))
        && components.all(|component| matches!(component, Component::Normal(_)))
}

fn preflight_plan<'a>(
    fixtures_root: &Path,
    corpus_root: &Path,
    plan: &'a CorpusPlan,
) -> Result<PreparedCorpus<'a>, XtaskError> {
    let corpus_relative = corpus_root.strip_prefix(fixtures_root).map_err(|_| {
        XtaskError::VerifyFailed(format!(
            "corpus root {} is outside fixtures root {}",
            corpus_root.display(),
            fixtures_root.display()
        ))
    })?;
    if !is_safe_relative_path(corpus_relative) {
        return Err(XtaskError::VerifyFailed(format!(
            "unsafe corpus root {}",
            corpus_root.display()
        )));
    }

    let manifest = manifest_json(&plan.manifest)?;
    let mut expected = BTreeSet::new();
    let mut files = Vec::with_capacity(plan.managed_files.len());
    for (relative, contents) in &plan.managed_files {
        if !is_safe_relative_path(relative) {
            return Err(XtaskError::VerifyFailed(format!(
                "unsafe managed corpus fixture path {}",
                relative.display()
            )));
        }
        let staged_relative = relative.strip_prefix(corpus_relative).map_err(|_| {
            XtaskError::VerifyFailed(format!(
                "managed corpus fixture {} is outside corpus root {}",
                relative.display(),
                corpus_relative.display()
            ))
        })?;
        if !is_safe_relative_path(staged_relative)
            || !staged_relative.starts_with("sources")
            || staged_relative.extension() != Some(OsStr::new("mmd"))
        {
            return Err(XtaskError::VerifyFailed(format!(
                "invalid managed corpus fixture path {}",
                relative.display()
            )));
        }
        expected.insert(relative.clone());
        files.push((staged_relative.to_path_buf(), contents.as_slice()));
    }

    let manifest_relative = corpus_relative.join(MANIFEST_FILE_NAME);
    let mut allowed = expected.clone();
    allowed.insert(manifest_relative.clone());
    let actual = corpus_files(fixtures_root, corpus_root)?;
    let stale: Vec<_> = actual.difference(&allowed).collect();
    if !stale.is_empty() {
        return Err(XtaskError::VerifyFailed(format!(
            "stale managed corpus files require review:\n{}",
            stale
                .iter()
                .map(|path| format!("  {}", path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        )));
    }

    for (relative, expected_contents) in &plan.managed_files {
        let path = fixtures_root.join(relative);
        match fs::read(&path) {
            Ok(existing) if existing == *expected_contents => {}
            Ok(_) => {
                return Err(XtaskError::VerifyFailed(format!(
                    "refusing to overwrite drifted corpus fixture {}",
                    path.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                });
            }
        }
    }

    let manifest_path = fixtures_root.join(manifest_relative);
    match fs::read(&manifest_path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(XtaskError::ReadFile {
                path: manifest_path.display().to_string(),
                source,
            });
        }
    }

    Ok(PreparedCorpus { manifest, files })
}

fn stage_corpus(
    corpus_root: &Path,
    prepared: &PreparedCorpus<'_>,
) -> Result<StagedCorpus, XtaskError> {
    let parent = corpus_root.parent().ok_or_else(|| {
        XtaskError::VerifyFailed(format!(
            "corpus root has no parent: {}",
            corpus_root.display()
        ))
    })?;
    let file_name = corpus_root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            XtaskError::VerifyFailed(format!(
                "corpus directory name is not UTF-8: {}",
                corpus_root.display()
            ))
        })?;
    fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;

    let staged = loop {
        let sequence = CORPUS_TRANSACTION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let suffix = format!("{}.{}", std::process::id(), sequence);
        let staging_path = parent.join(format!(".{file_name}.{suffix}.staging"));
        let backup_path = parent.join(format!(".{file_name}.{suffix}.backup"));
        match fs::symlink_metadata(&backup_path) {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(XtaskError::ReadFile {
                    path: backup_path.display().to_string(),
                    source,
                });
            }
        }
        match fs::create_dir(&staging_path) {
            Ok(()) => {
                break StagedCorpus {
                    staging_path,
                    backup_path,
                };
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(XtaskError::WriteFile {
                    path: staging_path.display().to_string(),
                    source,
                });
            }
        }
    };

    for (relative, contents) in &prepared.files {
        let path = staged.staging_path.join(relative);
        if let Err(error) = write_staged_file(&path, contents) {
            return Err(corpus_transaction_error(
                format!("failed to stage corpus fixture {}: {error}", path.display()),
                cleanup_staging_directory(&staged.staging_path),
            ));
        }
    }

    let manifest_path = staged.staging_path.join(MANIFEST_FILE_NAME);
    if let Err(error) = write_staged_file(&manifest_path, &prepared.manifest) {
        return Err(corpus_transaction_error(
            format!(
                "failed to stage corpus manifest {}: {error}",
                manifest_path.display()
            ),
            cleanup_staging_directory(&staged.staging_path),
        ));
    }

    Ok(staged)
}

fn write_staged_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("staged corpus file has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(contents)?;
    file.sync_all()
}

fn install_staged_corpus<R>(
    corpus_root: &Path,
    staged: &StagedCorpus,
    mut rename: R,
) -> Result<(), XtaskError>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
{
    let had_original = match fs::symlink_metadata(corpus_root) {
        Ok(metadata) if metadata.file_type().is_dir() => true,
        Ok(_) => {
            return Err(corpus_transaction_error(
                format!(
                    "corpus target is no longer a directory: {}",
                    corpus_root.display()
                ),
                cleanup_staging_directory(&staged.staging_path),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(corpus_transaction_error(
                format!(
                    "failed to inspect corpus target {} before install: {source}",
                    corpus_root.display()
                ),
                cleanup_staging_directory(&staged.staging_path),
            ));
        }
    };

    if had_original && let Err(source) = rename(corpus_root, &staged.backup_path) {
        return Err(corpus_transaction_error(
            format!(
                "failed to stage existing corpus {} at {}: {source}",
                corpus_root.display(),
                staged.backup_path.display()
            ),
            cleanup_staging_directory(&staged.staging_path),
        ));
    }

    if let Err(source) = rename(&staged.staging_path, corpus_root) {
        let mut cleanup_errors = Vec::new();
        if had_original && let Err(error) = rename(&staged.backup_path, corpus_root) {
            cleanup_errors.push(format!(
                "failed to restore corpus {} from {}: {error}",
                corpus_root.display(),
                staged.backup_path.display()
            ));
        }
        cleanup_errors.extend(cleanup_staging_directory(&staged.staging_path));
        return Err(corpus_transaction_error(
            format!(
                "failed to atomically install staged corpus {} at {}: {source}",
                staged.staging_path.display(),
                corpus_root.display()
            ),
            cleanup_errors,
        ));
    }

    if had_original && let Err(error) = fs::remove_dir_all(&staged.backup_path) {
        return Err(XtaskError::VerifyFailed(format!(
            "installed corpus {} but failed to remove committed backup {}: {error}",
            corpus_root.display(),
            staged.backup_path.display()
        )));
    }
    Ok(())
}

fn cleanup_staging_directory(path: &Path) -> Vec<String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Vec::new(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => vec![format!(
            "failed to remove staged corpus {}: {error}",
            path.display()
        )],
    }
}

fn corpus_transaction_error(primary: String, cleanup_errors: Vec<String>) -> XtaskError {
    if cleanup_errors.is_empty() {
        return XtaskError::VerifyFailed(primary);
    }
    XtaskError::VerifyFailed(format!(
        "{primary}; corpus rollback/cleanup errors: {}",
        cleanup_errors.join("; ")
    ))
}

fn check_plan(
    fixtures_root: &Path,
    corpus_root: &Path,
    plan: &CorpusPlan,
) -> Result<(), XtaskError> {
    let mut failures = Vec::new();
    let expected_manifest = manifest_json(&plan.manifest)?;
    let manifest_path = corpus_root.join(MANIFEST_FILE_NAME);
    match fs::read(&manifest_path) {
        Ok(actual) if actual == expected_manifest => {}
        Ok(_) => failures.push(format!("manifest drift: {}", manifest_path.display())),
        Err(error) => failures.push(format!(
            "missing manifest {}: {error}",
            manifest_path.display()
        )),
    }

    for (relative, expected) in &plan.managed_files {
        let path = fixtures_root.join(relative);
        match fs::read(&path) {
            Ok(actual) if actual == *expected => {}
            Ok(_) => failures.push(format!("fixture drift: {}", path.display())),
            Err(error) => failures.push(format!("missing fixture {}: {error}", path.display())),
        }
    }

    let expected: BTreeSet<_> = plan.managed_files.keys().cloned().collect();
    let actual = managed_mmd_files(fixtures_root, corpus_root)?;
    failures.extend(
        actual
            .difference(&expected)
            .map(|path| format!("stale managed fixture: {}", path.display())),
    );

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::VerifyFailed(failures.join("\n")))
    }
}

fn managed_mmd_files(
    fixtures_root: &Path,
    corpus_root: &Path,
) -> Result<BTreeSet<PathBuf>, XtaskError> {
    let sources_relative = corpus_root
        .join("sources")
        .strip_prefix(fixtures_root)
        .expect("corpus sources stay below fixtures root")
        .to_path_buf();
    Ok(corpus_files(fixtures_root, corpus_root)?
        .into_iter()
        .filter(|path| {
            path.starts_with(&sources_relative) && path.extension() == Some(OsStr::new("mmd"))
        })
        .collect())
}

fn corpus_files(fixtures_root: &Path, corpus_root: &Path) -> Result<BTreeSet<PathBuf>, XtaskError> {
    match fs::symlink_metadata(corpus_root) {
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            return Err(XtaskError::VerifyFailed(format!(
                "corpus root is not a directory: {}",
                corpus_root.display()
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(source) => {
            return Err(XtaskError::ReadFile {
                path: corpus_root.display().to_string(),
                source,
            });
        }
    }

    let mut paths = Vec::new();
    collect_all_corpus_paths(corpus_root, &mut paths)?;
    paths
        .into_iter()
        .map(|path| {
            path.strip_prefix(fixtures_root)
                .map(Path::to_path_buf)
                .map_err(|_| {
                    XtaskError::VerifyFailed(format!(
                        "corpus file {} escaped fixtures root {}",
                        path.display(),
                        fixtures_root.display()
                    ))
                })
        })
        .collect()
}

fn collect_all_corpus_paths(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), XtaskError> {
    for entry in fs::read_dir(directory).map_err(|source| XtaskError::ReadFile {
        path: directory.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| XtaskError::ReadFile {
            path: directory.display().to_string(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        if file_type.is_dir() {
            collect_all_corpus_paths(&path, paths)?;
        } else if file_type.is_file() {
            paths.push(path);
        } else {
            return Err(XtaskError::VerifyFailed(format!(
                "unsupported corpus filesystem entry requires review: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn manifest_json(manifest: &CorpusManifest) -> Result<Vec<u8>, XtaskError> {
    let mut bytes = serde_json::to_vec_pretty(manifest)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestRoot(PathBuf);

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn source(path: &str, contents: &[u8]) -> SourceFile {
        SourceFile {
            path: path.to_string(),
            contents: contents.to_vec(),
            sha256: sha256_hex(contents),
        }
    }

    fn options() -> Options {
        Options {
            from_ref: "mermaid@11.15.0".to_string(),
            to_ref: "mermaid@11.16.0".to_string(),
            corpus_name: "mermaid-11.16.0".to_string(),
            check: false,
        }
    }

    fn test_corpus(label: &str) -> (TestRoot, PathBuf, PathBuf, PathBuf) {
        let sequence = TEST_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "merman-upstream-mmd-corpus-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create corpus test root");
        let fixtures_root = root.join("fixtures");
        let corpus_relative = PathBuf::from("_upstream").join(format!("{label}-{sequence}"));
        let corpus_root = fixtures_root.join(&corpus_relative);
        fs::create_dir_all(corpus_root.parent().expect("corpus parent"))
            .expect("create corpus parent");
        (TestRoot(root), fixtures_root, corpus_root, corpus_relative)
    }

    fn transaction_artifacts(corpus_root: &Path) -> Vec<PathBuf> {
        let parent = corpus_root.parent().expect("corpus parent");
        let prefix = format!(
            ".{}.",
            corpus_root
                .file_name()
                .and_then(|name| name.to_str())
                .expect("UTF-8 corpus name")
        );
        fs::read_dir(parent)
            .expect("read corpus parent")
            .map(|entry| entry.expect("read corpus sibling").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with(&prefix)
                            && (name.ends_with(".staging") || name.ends_with(".backup"))
                    })
            })
            .collect()
    }

    #[test]
    fn plan_copies_every_source_path_even_when_contents_match() {
        let existing_contents = b"flowchart LR\nA-->B\n";
        let missing_contents = b"sequenceDiagram\nA->>B: hi\n";
        let sources = vec![
            source("tests/a.mmd", existing_contents),
            source("tests/b.mmd", missing_contents),
            source("tests/c.mmd", missing_contents),
        ];

        let plan = build_plan(
            sources,
            Path::new("_upstream/mermaid-11.16.0"),
            &options(),
            "abc123".to_string(),
        );

        assert_eq!(plan.manifest.summary.source_file_count, 3);
        assert_eq!(plan.manifest.summary.unique_content_count, 2);
        assert_eq!(plan.manifest.summary.managed_file_count, 3);
        assert_eq!(
            plan.manifest.entries["tests/a.mmd"].fixture,
            "_upstream/mermaid-11.16.0/sources/tests/a.mmd"
        );
        assert_eq!(
            plan.manifest.entries["tests/b.mmd"].fixture,
            "_upstream/mermaid-11.16.0/sources/tests/b.mmd"
        );
        assert_eq!(
            plan.manifest.entries["tests/c.mmd"].fixture,
            "_upstream/mermaid-11.16.0/sources/tests/c.mmd"
        );
    }

    #[test]
    fn default_name_is_stable_for_mermaid_tags() {
        assert_eq!(default_corpus_name("mermaid@11.16.0"), "mermaid-11.16.0");
    }

    #[test]
    fn source_paths_must_be_relative_mmd_files() {
        assert!(validate_source_path("cypress/example.mmd").is_ok());
        assert!(validate_source_path("../outside.mmd").is_err());
        assert!(validate_source_path("/tmp/example.mmd").is_err());
        assert!(validate_source_path("cypress/example.svg").is_err());
    }

    #[test]
    fn later_drift_rejection_does_not_install_earlier_new_fixture() {
        let (_root, fixtures_root, corpus_root, corpus_relative) = test_corpus("preflight-drift");
        let drifted_path = corpus_root.join("sources/tests/z-drift.mmd");
        fs::create_dir_all(drifted_path.parent().expect("drifted fixture parent"))
            .expect("create drifted fixture parent");
        fs::write(&drifted_path, b"flowchart LR\nA-->C\n").expect("write drifted fixture");
        let plan = build_plan(
            vec![
                source("tests/a-new.mmd", b"flowchart LR\nA-->B\n"),
                source("tests/z-drift.mmd", b"flowchart LR\nA-->B\n"),
            ],
            &corpus_relative,
            &options(),
            "abc123".to_string(),
        );

        let error = write_plan(&fixtures_root, &corpus_root, &plan)
            .expect_err("drifted fixture must reject corpus install");

        assert!(error.to_string().contains("refusing to overwrite drifted"));
        assert!(!corpus_root.join("sources/tests/a-new.mmd").exists());
        assert_eq!(
            fs::read(&drifted_path).expect("read preserved drifted fixture"),
            b"flowchart LR\nA-->C\n"
        );
        assert!(!corpus_root.join(MANIFEST_FILE_NAME).exists());
        assert!(transaction_artifacts(&corpus_root).is_empty());
    }

    #[test]
    fn successful_install_replaces_the_complete_corpus_directory() {
        let (_root, fixtures_root, corpus_root, corpus_relative) =
            test_corpus("complete-replacement");
        let existing_path = corpus_root.join("sources/tests/existing.mmd");
        fs::create_dir_all(existing_path.parent().expect("existing fixture parent"))
            .expect("create existing fixture parent");
        fs::write(&existing_path, b"flowchart LR\nA-->B\n").expect("write existing fixture");
        fs::write(corpus_root.join(MANIFEST_FILE_NAME), b"old manifest\n")
            .expect("write old manifest");
        let obsolete_directory = corpus_root.join("sources/obsolete/empty");
        fs::create_dir_all(&obsolete_directory).expect("create obsolete empty directory");
        let plan = build_plan(
            vec![
                source("tests/existing.mmd", b"flowchart LR\nA-->B\n"),
                source("tests/new.mmd", b"sequenceDiagram\nA->>B: hi\n"),
            ],
            &corpus_relative,
            &options(),
            "abc123".to_string(),
        );

        write_plan(&fixtures_root, &corpus_root, &plan).expect("install complete corpus");

        check_plan(&fixtures_root, &corpus_root, &plan).expect("check installed corpus");
        assert_eq!(
            fs::read(corpus_root.join(MANIFEST_FILE_NAME)).expect("read installed manifest"),
            manifest_json(&plan.manifest).expect("encode expected manifest")
        );
        assert!(!obsolete_directory.exists());
        assert!(transaction_artifacts(&corpus_root).is_empty());
    }

    #[test]
    fn failed_directory_install_restores_the_previous_corpus() {
        let (_root, fixtures_root, corpus_root, corpus_relative) = test_corpus("install-rollback");
        let existing_path = corpus_root.join("sources/tests/existing.mmd");
        fs::create_dir_all(existing_path.parent().expect("existing fixture parent"))
            .expect("create existing fixture parent");
        fs::write(&existing_path, b"flowchart LR\nA-->B\n").expect("write existing fixture");
        let old_manifest = b"old manifest\n";
        fs::write(corpus_root.join(MANIFEST_FILE_NAME), old_manifest).expect("write old manifest");
        let plan = build_plan(
            vec![
                source("tests/existing.mmd", b"flowchart LR\nA-->B\n"),
                source("tests/new.mmd", b"sequenceDiagram\nA->>B: hi\n"),
            ],
            &corpus_relative,
            &options(),
            "abc123".to_string(),
        );
        let mut rejected_install = false;

        let error = write_plan_with_renamer(&fixtures_root, &corpus_root, &plan, |from, to| {
            let is_staging_install = to == corpus_root
                && from
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".staging"));
            if is_staging_install && !rejected_install {
                rejected_install = true;
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected corpus install failure",
                ));
            }
            fs::rename(from, to)
        })
        .expect_err("injected install failure must reject transaction");

        assert!(rejected_install);
        assert!(error.to_string().contains("failed to atomically install"));
        assert_eq!(
            fs::read(&existing_path).expect("read restored fixture"),
            b"flowchart LR\nA-->B\n"
        );
        assert_eq!(
            fs::read(corpus_root.join(MANIFEST_FILE_NAME)).expect("read restored manifest"),
            old_manifest
        );
        assert!(!corpus_root.join("sources/tests/new.mmd").exists());
        assert!(transaction_artifacts(&corpus_root).is_empty());
    }

    #[test]
    fn committed_mermaid_11_16_corpus_is_complete_and_content_addressed() {
        let fixtures_root = crate::cmd::fixtures_root();
        let corpus_root = fixtures_root.join("_upstream/mermaid-11.16.0");
        let manifest_path = corpus_root.join(MANIFEST_FILE_NAME);
        let manifest: CorpusManifest = serde_json::from_slice(
            &fs::read(&manifest_path).expect("read Mermaid 11.16 corpus manifest"),
        )
        .expect("parse Mermaid 11.16 corpus manifest");

        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.source.repository, "repo-ref/mermaid");
        assert_eq!(manifest.source.from_ref, "mermaid@11.15.0");
        assert_eq!(manifest.source.to_ref, "mermaid@11.16.0");
        assert_eq!(
            manifest.source.to_commit,
            "7c0cafcf42e76bfaf79d0cbbd12edb986612f014"
        );
        assert_eq!(manifest.entries.len(), 122);
        assert_eq!(manifest.summary.source_file_count, manifest.entries.len());

        let unique_hashes: BTreeSet<_> = manifest
            .entries
            .values()
            .map(|entry| entry.sha256.as_str())
            .collect();
        assert_eq!(manifest.summary.unique_content_count, unique_hashes.len());

        let mut referenced_managed = BTreeSet::new();
        for (source_path, entry) in &manifest.entries {
            validate_source_path(source_path).expect("safe upstream source path");
            let fixture_path = Path::new(&entry.fixture);
            assert!(
                !fixture_path.components().any(|component| matches!(
                    component,
                    Component::Prefix(_) | Component::RootDir | Component::ParentDir
                )),
                "unsafe fixture path: {}",
                fixture_path.display()
            );
            let contents = fs::read(fixtures_root.join(fixture_path))
                .unwrap_or_else(|error| panic!("read {}: {error}", fixture_path.display()));
            assert_eq!(
                sha256_hex(&contents),
                entry.sha256,
                "content drift for {}",
                fixture_path.display()
            );

            assert!(fixture_path.starts_with("_upstream/mermaid-11.16.0/sources"));
            referenced_managed.insert(fixture_path.to_path_buf());
        }

        assert_eq!(manifest.summary.managed_file_count, 122);
        assert_eq!(
            manifest.summary.managed_file_count,
            referenced_managed.len()
        );
        let actual_managed =
            managed_mmd_files(&fixtures_root, &corpus_root).expect("enumerate managed corpus");
        assert_eq!(actual_managed, referenced_managed);
    }
}
