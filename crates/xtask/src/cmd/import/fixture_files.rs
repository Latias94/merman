use crate::XtaskError;
use merman_fixture_render_context::{
    FixtureRenderContext, MANIFEST_RELATIVE_PATH, RenderContextCatalog,
};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static RENDER_CONTEXT_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn fixtures_root() -> PathBuf {
    crate::cmd::fixtures_root()
}

fn upstream_svg_path_in(root: &Path, diagram_dir: &str, stem: &str) -> PathBuf {
    root.join("upstream-svgs")
        .join(diagram_dir)
        .join(format!("{stem}.svg"))
}

fn upstream_svg_path(diagram_dir: &str, stem: &str) -> PathBuf {
    upstream_svg_path_in(&fixtures_root(), diagram_dir, stem)
}

fn deferred_fixture_dir_in(root: &Path, diagram_dir: &str) -> PathBuf {
    root.join("_deferred").join(diagram_dir)
}

fn deferred_fixture_path_in(root: &Path, diagram_dir: &str, stem: &str) -> PathBuf {
    deferred_fixture_dir_in(root, diagram_dir).join(format!("{stem}.mmd"))
}

fn deferred_fixture_path(diagram_dir: &str, stem: &str) -> PathBuf {
    deferred_fixture_path_in(&fixtures_root(), diagram_dir, stem)
}

fn deferred_upstream_svg_dir_in(root: &Path, diagram_dir: &str) -> PathBuf {
    root.join("_deferred")
        .join("upstream-svgs")
        .join(diagram_dir)
}

fn deferred_upstream_svg_path_in(root: &Path, diagram_dir: &str, stem: &str) -> PathBuf {
    deferred_upstream_svg_dir_in(root, diagram_dir).join(format!("{stem}.svg"))
}

fn deferred_upstream_svg_path(diagram_dir: &str, stem: &str) -> PathBuf {
    deferred_upstream_svg_path_in(&fixtures_root(), diagram_dir, stem)
}

fn golden_json_path_in(root: &Path, diagram_dir: &str, stem: &str) -> PathBuf {
    root.join(diagram_dir).join(format!("{stem}.golden.json"))
}

fn golden_json_path(diagram_dir: &str, stem: &str) -> PathBuf {
    golden_json_path_in(&fixtures_root(), diagram_dir, stem)
}

fn layout_golden_json_path_in(root: &Path, diagram_dir: &str, stem: &str) -> PathBuf {
    root.join(diagram_dir)
        .join(format!("{stem}.layout.golden.json"))
}

fn layout_golden_json_path(diagram_dir: &str, stem: &str) -> PathBuf {
    layout_golden_json_path_in(&fixtures_root(), diagram_dir, stem)
}

fn render_contexts_path_in(root: &Path) -> PathBuf {
    root.join(MANIFEST_RELATIVE_PATH)
}

fn render_contexts_path() -> PathBuf {
    render_contexts_path_in(&fixtures_root())
}

fn fixture_relative_path(diagram_dir: &str, stem: &str) -> String {
    format!("{diagram_dir}/{stem}.mmd")
}

#[derive(Clone, Debug)]
struct ImportedFileSnapshot {
    path: PathBuf,
    contents: Option<Vec<u8>>,
}

impl ImportedFileSnapshot {
    fn capture(path: PathBuf) -> Result<Self, XtaskError> {
        let contents = match fs::read(&path) {
            Ok(contents) => Some(contents),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        Ok(Self { path, contents })
    }

    fn rollback(&self) -> Result<(), String> {
        match &self.contents {
            Some(contents) => {
                if let Some(parent) = self.path.parent() {
                    fs::create_dir_all(parent).map_err(|err| {
                        format!(
                            "failed to recreate imported fixture directory {} during rollback: {err}",
                            parent.display()
                        )
                    })?;
                }
                fs::write(&self.path, contents).map_err(|err| {
                    format!(
                        "failed to restore imported fixture file {} during rollback: {err}",
                        self.path.display()
                    )
                })
            }
            None => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(format!(
                    "failed to remove imported fixture file {} during rollback: {err}",
                    self.path.display()
                )),
            },
        }
    }
}

#[derive(Clone, Debug)]
struct ImportedDirectorySnapshot {
    path: PathBuf,
    existed: bool,
    files: Vec<ImportedFileSnapshot>,
}

impl ImportedDirectorySnapshot {
    fn capture(path: PathBuf) -> Result<Self, XtaskError> {
        let existed = match fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => true,
            Ok(_) => {
                return Err(XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source: std::io::Error::other("upstream SVG family path is not a directory"),
                });
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => false,
            Err(source) => {
                return Err(XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                });
            }
        };

        let mut files = Vec::new();
        if existed {
            let entries = fs::read_dir(&path).map_err(|source| XtaskError::ReadFile {
                path: path.display().to_string(),
                source,
            })?;
            for entry in entries {
                let entry = entry.map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })?;
                let entry_path = entry.path();
                if is_managed_upstream_family_path(&entry_path) {
                    files.push(ImportedFileSnapshot::capture(entry_path)?);
                }
            }
            files.sort_by(|left, right| left.path.cmp(&right.path));
        }

        Ok(Self {
            path,
            existed,
            files,
        })
    }

    fn rollback(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut directory_exists = match fs::metadata(&self.path) {
            Ok(metadata) if metadata.is_dir() => true,
            Ok(_) => {
                errors.push(format!(
                    "failed to restore upstream SVG family {}: path is not a directory",
                    self.path.display()
                ));
                return errors;
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
            Err(err) => {
                errors.push(format!(
                    "failed to inspect upstream SVG family {} during rollback: {err}",
                    self.path.display()
                ));
                return errors;
            }
        };

        if !self.files.is_empty() && !directory_exists {
            if let Err(err) = fs::create_dir_all(&self.path) {
                errors.push(format!(
                    "failed to recreate upstream SVG family directory {} during rollback: {err}",
                    self.path.display()
                ));
                return errors;
            }
            directory_exists = true;
        }

        let captured: BTreeSet<&Path> = self.files.iter().map(|file| file.path.as_path()).collect();
        if directory_exists {
            match fs::read_dir(&self.path) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                let entry_path = entry.path();
                                if is_managed_upstream_family_path(&entry_path)
                                    && !captured.contains(entry_path.as_path())
                                    && let Err(err) = fs::remove_file(&entry_path)
                                {
                                    errors.push(format!(
                                        "failed to remove imported upstream family file {} during rollback: {err}",
                                        entry_path.display()
                                    ));
                                }
                            }
                            Err(err) => errors.push(format!(
                                "failed to enumerate upstream SVG family {} during rollback: {err}",
                                self.path.display()
                            )),
                        }
                    }
                }
                Err(err) => errors.push(format!(
                    "failed to enumerate upstream SVG family {} during rollback: {err}",
                    self.path.display()
                )),
            }
        }

        errors.extend(
            self.files
                .iter()
                .filter_map(|snapshot| snapshot.rollback().err()),
        );

        if !self.existed && directory_exists {
            match fs::remove_dir(&self.path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) if err.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                Err(err) => errors.push(format!(
                    "failed to remove imported upstream family directory {} during rollback: {err}",
                    self.path.display()
                )),
            }
        }

        errors
    }
}

fn is_managed_upstream_family_path(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "svg")
        || path
            .file_name()
            .is_some_and(|name| name == "_baseline-manifest.json" || name == "_failures.txt")
}

#[derive(Clone, Debug)]
pub(crate) struct ImportedFixtureSnapshot {
    active_files: Vec<ImportedFileSnapshot>,
    deferred_files: Vec<ImportedFileSnapshot>,
    upstream_family: ImportedDirectorySnapshot,
    render_context_file: ImportedFileSnapshot,
    render_context_catalog: RenderContextCatalog,
    render_context_relative_path: String,
    render_context_value: Option<FixtureRenderContext>,
}

impl ImportedFixtureSnapshot {
    pub(crate) fn capture(
        diagram_dir: &str,
        stem: &str,
        fixture_path: &Path,
    ) -> Result<Self, XtaskError> {
        Self::capture_in(&fixtures_root(), diagram_dir, stem, fixture_path)
    }

    fn capture_in(
        root: &Path,
        diagram_dir: &str,
        stem: &str,
        fixture_path: &Path,
    ) -> Result<Self, XtaskError> {
        let active_files = [
            fixture_path.to_path_buf(),
            golden_json_path_in(root, diagram_dir, stem),
            layout_golden_json_path_in(root, diagram_dir, stem),
        ]
        .into_iter()
        .map(ImportedFileSnapshot::capture)
        .collect::<Result<Vec<_>, _>>()?;
        let deferred_files = [
            deferred_fixture_path_in(root, diagram_dir, stem),
            deferred_upstream_svg_path_in(root, diagram_dir, stem),
        ]
        .into_iter()
        .map(ImportedFileSnapshot::capture)
        .collect::<Result<Vec<_>, _>>()?;
        let upstream_family =
            ImportedDirectorySnapshot::capture(root.join("upstream-svgs").join(diagram_dir))?;

        let render_context_path = render_contexts_path_in(root);
        let render_context_file = ImportedFileSnapshot::capture(render_context_path)?;
        let render_context_relative_path = fixture_relative_path(diagram_dir, stem);
        let render_context_catalog =
            RenderContextCatalog::load_for_update(root).map_err(render_context_catalog_error)?;
        let render_context_value = render_context_catalog
            .context_for_relative_fixture(&render_context_relative_path)
            .map_err(render_context_catalog_error)?
            .cloned();

        Ok(Self {
            active_files,
            deferred_files,
            upstream_family,
            render_context_file,
            render_context_catalog,
            render_context_relative_path,
            render_context_value,
        })
    }

    pub(crate) fn rollback(&self) -> Vec<String> {
        self.rollback_inner(true)
    }

    pub(crate) fn rollback_preserving_deferred(&self) -> Vec<String> {
        self.rollback_inner(false)
    }

    fn rollback_inner(&self, restore_deferred: bool) -> Vec<String> {
        let mut errors = self.upstream_family.rollback();
        errors.extend(
            self.active_files
                .iter()
                .filter_map(|snapshot| snapshot.rollback().err()),
        );
        if restore_deferred {
            errors.extend(
                self.deferred_files
                    .iter()
                    .filter_map(|snapshot| snapshot.rollback().err()),
            );
        }
        if let Err(err) = self.rollback_render_context() {
            errors.push(err);
        }
        errors
    }

    fn rollback_render_context(&self) -> Result<(), String> {
        let fixtures_root = self
            .render_context_file
            .path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| {
                format!(
                    "fixture render context manifest has no fixtures root: {}",
                    self.render_context_file.path.display()
                )
            })?;
        let mut catalog = match RenderContextCatalog::load_for_fixture_update(
            fixtures_root,
            &self.render_context_relative_path,
        ) {
            Ok(catalog) => catalog,
            Err(_) => return self.render_context_file.rollback(),
        };
        let current_value = catalog
            .context_for_relative_fixture(&self.render_context_relative_path)
            .map_err(|error| error.to_string())?
            .cloned();
        let candidate_changed = current_value != self.render_context_value;
        if candidate_changed {
            match &self.render_context_value {
                Some(_) => {
                    let source_path = fixtures_root.join(&self.render_context_relative_path);
                    let source = fs::read(&source_path).map_err(|error| {
                        format!(
                            "failed to read restored fixture {} during render context rollback: {error}",
                            source_path.display()
                        )
                    })?;
                    catalog
                        .upsert_from_source(&self.render_context_relative_path, &source)
                        .map_err(|error| error.to_string())?;
                }
                None => {
                    catalog
                        .remove(&self.render_context_relative_path)
                        .map_err(|error| error.to_string())?;
                }
            }
        }

        let current_semantics = catalog.to_json().map_err(|error| error.to_string())?;
        let snapshot_semantics = self
            .render_context_catalog
            .to_json()
            .map_err(|error| error.to_string())?;
        if current_semantics == snapshot_semantics {
            return self.render_context_file.rollback();
        }
        if !candidate_changed {
            return Ok(());
        }

        if self.render_context_file.contents.is_none() && catalog.contexts().next().is_none() {
            return match fs::remove_file(&self.render_context_file.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(format!(
                    "failed to remove fixture render context manifest {} during rollback: {err}",
                    self.render_context_file.path.display()
                )),
            };
        }

        write_render_contexts_to(&self.render_context_file.path, &catalog)
            .map_err(|error| error.to_string())
    }
}

fn config_look_from_yaml(value: &serde_json::Value) -> Option<&str> {
    let mapping = value.as_object()?;
    if let Some(look) = mapping.get("look").and_then(serde_json::Value::as_str) {
        return Some(look);
    }

    mapping
        .get("config")
        .and_then(serde_json::Value::as_object)
        .and_then(|config| config.get("look"))
        .and_then(serde_json::Value::as_str)
}

fn split_yaml_frontmatter(input: &str) -> Option<(&str, &str)> {
    let after_marker = input.strip_prefix("---")?;
    let open_line_end = after_marker.find('\n')?;
    if !after_marker[..open_line_end].trim().is_empty() {
        return None;
    }

    let body_start = 3 + open_line_end + 1;
    let rest = &input[body_start..];
    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        let without_newline = line.trim_end_matches(['\r', '\n']);
        if without_newline.trim() == "---" {
            let body = &rest[..offset];
            let stripped = &rest[offset + line.len()..];
            return Some((body, stripped));
        }
        offset += line.len();
    }
    None
}

pub(crate) fn imported_fixture_config_look(body: &str) -> Option<String> {
    let (yaml, _) = split_yaml_frontmatter(body)?;
    let parsed = serde_saphyr::from_str::<serde_json::Value>(yaml).ok()?;
    config_look_from_yaml(&parsed).map(str::to_string)
}

fn render_context_catalog_error(error: merman_fixture_render_context::CatalogError) -> XtaskError {
    XtaskError::SnapshotUpdateFailed(error.to_string())
}

fn write_render_contexts(catalog: &RenderContextCatalog) -> Result<(), XtaskError> {
    write_render_contexts_to(&render_contexts_path(), catalog)
}

fn write_render_contexts_to(path: &Path, catalog: &RenderContextCatalog) -> Result<(), XtaskError> {
    write_render_contexts_to_with_backup_remover(path, catalog, |backup_path| {
        fs::remove_file(backup_path)
    })
}

fn write_render_contexts_to_with_backup_remover<R>(
    path: &Path,
    catalog: &RenderContextCatalog,
    remove_backup: R,
) -> Result<(), XtaskError>
where
    R: FnOnce(&Path) -> std::io::Result<()>,
{
    let pretty = catalog.to_json().map_err(render_context_catalog_error)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let sequence = RENDER_CONTEXT_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("render_contexts.json");
    let transaction_suffix = format!("{}-{sequence}", std::process::id());
    let temp_path = path.with_file_name(format!(".{file_name}.{transaction_suffix}.tmp"));
    let backup_path = path.with_file_name(format!(".{file_name}.{transaction_suffix}.backup"));

    let write_temp_result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)?;
        file.write_all(pretty.as_bytes())?;
        file.sync_all()
    })();
    if let Err(source) = write_temp_result {
        let _ = fs::remove_file(&temp_path);
        return Err(XtaskError::WriteFile {
            path: temp_path.display().to_string(),
            source,
        });
    }

    let had_original = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => true,
        Ok(_) => {
            let _ = fs::remove_file(&temp_path);
            return Err(XtaskError::WriteFile {
                path: path.display().to_string(),
                source: std::io::Error::other("render context manifest path is not a file"),
            });
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => false,
        Err(source) => {
            let _ = fs::remove_file(&temp_path);
            return Err(XtaskError::ReadFile {
                path: path.display().to_string(),
                source,
            });
        }
    };

    if had_original {
        fs::rename(path, &backup_path).map_err(|source| {
            let _ = fs::remove_file(&temp_path);
            XtaskError::WriteFile {
                path: backup_path.display().to_string(),
                source,
            }
        })?;
    }

    if let Err(source) = fs::rename(&temp_path, path) {
        let mut rollback_error = None;
        if had_original && let Err(error) = fs::rename(&backup_path, path) {
            rollback_error = Some(error);
        }
        let _ = fs::remove_file(&temp_path);
        return match rollback_error {
            Some(rollback_error) => Err(XtaskError::WriteFile {
                path: path.display().to_string(),
                source: std::io::Error::other(format!(
                    "failed to install render context manifest: {source}; failed to restore backup: {rollback_error}"
                )),
            }),
            None => Err(XtaskError::WriteFile {
                path: path.display().to_string(),
                source,
            }),
        };
    }

    if had_original && let Err(err) = remove_backup(&backup_path) {
        eprintln!(
            "warning: failed to remove committed fixture render context backup {}: {err}",
            backup_path.display()
        );
    }
    Ok(())
}

fn update_render_context(diagram_dir: &str, stem: &str, body: &str) -> Result<(), XtaskError> {
    let relative_path = fixture_relative_path(diagram_dir, stem);
    let root = fixtures_root();
    let mut catalog = RenderContextCatalog::load_for_fixture_update(&root, &relative_path)
        .map_err(render_context_catalog_error)?;
    if catalog
        .upsert_from_source(&relative_path, body.as_bytes())
        .map_err(render_context_catalog_error)?
    {
        write_render_contexts(&catalog)?;
    }
    Ok(())
}

fn remove_render_context(diagram_dir: &str, stem: &str) -> Result<(), XtaskError> {
    remove_render_context_from(&fixtures_root(), diagram_dir, stem)
}

fn remove_render_context_from(
    root: &Path,
    diagram_dir: &str,
    stem: &str,
) -> Result<(), XtaskError> {
    let relative_path = fixture_relative_path(diagram_dir, stem);
    let mut catalog = RenderContextCatalog::load_for_fixture_update(root, &relative_path)
        .map_err(render_context_catalog_error)?;
    if catalog
        .remove(&relative_path)
        .map_err(render_context_catalog_error)?
    {
        write_render_contexts_to(&render_contexts_path_in(root), &catalog)?;
    }
    Ok(())
}

fn move_or_copy_then_remove(
    src: &Path,
    dst: &Path,
    replace_existing: bool,
) -> Result<(), XtaskError> {
    if dst.exists() && !replace_existing {
        return Err(XtaskError::WriteFile {
            path: dst.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "deferred fixture destination already exists",
            ),
        });
    }

    fs::copy(src, dst).map_err(|source| XtaskError::WriteFile {
        path: dst.display().to_string(),
        source,
    })?;
    fs::remove_file(src).map_err(|source| XtaskError::WriteFile {
        path: src.display().to_string(),
        source,
    })
}

fn remove_file_if_present(path: &Path) -> Result<(), XtaskError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(XtaskError::WriteFile {
            path: path.display().to_string(),
            source,
        }),
    }
}

fn rollback_failed_file_operation(
    error: XtaskError,
    snapshot: &ImportedFixtureSnapshot,
) -> XtaskError {
    let rollback_errors = snapshot.rollback();
    if rollback_errors.is_empty() {
        return error;
    }
    XtaskError::UpstreamSvgFailed(format!(
        "{error}; failed to roll back imported fixture file operation: {}",
        rollback_errors.join("; ")
    ))
}

pub(crate) fn cleanup_fixture_files(
    diagram_dir: &str,
    stem: &str,
    path: &Path,
) -> Result<(), XtaskError> {
    let snapshot = ImportedFixtureSnapshot::capture(diagram_dir, stem, path)?;
    let result = (|| {
        remove_file_if_present(path)?;
        remove_file_if_present(&upstream_svg_path(diagram_dir, stem))?;
        remove_file_if_present(&golden_json_path(diagram_dir, stem))?;
        remove_file_if_present(&layout_golden_json_path(diagram_dir, stem))?;
        remove_render_context(diagram_dir, stem)
    })();
    result.map_err(|error| rollback_failed_file_operation(error, &snapshot))
}

pub(crate) fn cleanup_deferred_fixture_files(
    diagram_dir: &str,
    stem: &str,
) -> Result<(), XtaskError> {
    let active_path = fixtures_root()
        .join(diagram_dir)
        .join(format!("{stem}.mmd"));
    let snapshot = ImportedFixtureSnapshot::capture(diagram_dir, stem, &active_path)?;
    let result = (|| {
        remove_file_if_present(&deferred_fixture_path(diagram_dir, stem))?;
        remove_file_if_present(&deferred_upstream_svg_path(diagram_dir, stem))
    })();
    result.map_err(|error| rollback_failed_file_operation(error, &snapshot))
}

pub(crate) fn write_imported_fixture(
    diagram_dir: &str,
    stem: &str,
    path: &Path,
    body: &str,
) -> Result<(), XtaskError> {
    fs::write(path, body.as_bytes()).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })?;
    update_render_context(diagram_dir, stem, body)
}

pub(crate) fn defer_fixture_files_with_replace_existing(
    diagram_dir: &str,
    stem: &str,
    path: &Path,
    keep_upstream_svg: bool,
    replace_existing: bool,
) -> Result<PathBuf, XtaskError> {
    defer_fixture_files_with_replace_existing_in(
        &fixtures_root(),
        diagram_dir,
        stem,
        path,
        keep_upstream_svg,
        replace_existing,
    )
}

fn defer_fixture_files_with_replace_existing_in(
    root: &Path,
    diagram_dir: &str,
    stem: &str,
    path: &Path,
    keep_upstream_svg: bool,
    replace_existing: bool,
) -> Result<PathBuf, XtaskError> {
    let snapshot = ImportedFixtureSnapshot::capture_in(root, diagram_dir, stem, path)?;
    let deferred_fixture_dir = deferred_fixture_dir_in(root, diagram_dir);
    let deferred_path = deferred_fixture_path_in(root, diagram_dir, stem);
    let result = (|| {
        fs::create_dir_all(&deferred_fixture_dir).map_err(|source| XtaskError::WriteFile {
            path: deferred_fixture_dir.display().to_string(),
            source,
        })?;
        move_or_copy_then_remove(path, &deferred_path, replace_existing)?;

        if keep_upstream_svg {
            let upstream_path = upstream_svg_path_in(root, diagram_dir, stem);
            let deferred_svg_dir = deferred_upstream_svg_dir_in(root, diagram_dir);
            fs::create_dir_all(&deferred_svg_dir).map_err(|source| XtaskError::WriteFile {
                path: deferred_svg_dir.display().to_string(),
                source,
            })?;

            let deferred_svg_path = deferred_upstream_svg_path_in(root, diagram_dir, stem);
            move_or_copy_then_remove(&upstream_path, &deferred_svg_path, replace_existing)?;
        } else {
            remove_file_if_present(&upstream_svg_path_in(root, diagram_dir, stem))?;
            remove_file_if_present(&deferred_upstream_svg_path_in(root, diagram_dir, stem))?;
        }

        remove_file_if_present(&golden_json_path_in(root, diagram_dir, stem))?;
        remove_file_if_present(&layout_golden_json_path_in(root, diagram_dir, stem))?;
        remove_render_context_from(root, diagram_dir, stem)
    })();
    result
        .map(|()| deferred_path)
        .map_err(|error| rollback_failed_file_operation(error, &snapshot))
}

#[cfg(test)]
mod tests {
    use super::{
        ImportedFixtureSnapshot, defer_fixture_files_with_replace_existing_in,
        deferred_fixture_path_in, deferred_upstream_svg_path_in, golden_json_path_in,
        imported_fixture_config_look, layout_golden_json_path_in, render_contexts_path_in,
        upstream_svg_path_in, write_render_contexts_to,
        write_render_contexts_to_with_backup_remover,
    };
    use merman_fixture_render_context::{RenderContextCatalog, SecurityLevel};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestFixtureRoot {
        path: PathBuf,
    }

    impl TestFixtureRoot {
        fn new() -> Self {
            let sequence = TEMP_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "merman-import-fixture-transaction-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated fixture transaction root");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestFixtureRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_test_file(path: &Path, contents: impl AsRef<[u8]>) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create test fixture parent directory");
        }
        fs::write(path, contents).expect("write test fixture file");
    }

    const LOOSE_SOURCE: &str = "%%{init: {\"securityLevel\":\"loose\"}}%%\nflowchart TD\n  A-->B\n";
    const SANDBOX_SOURCE: &str =
        "%%{init: {\"securityLevel\":\"sandbox\"}}%%\nflowchart TD\n  A-->B\n";

    fn catalog_from_sources(root: &Path, sources: &[(&str, &str)]) -> RenderContextCatalog {
        let mut catalog =
            RenderContextCatalog::rebuild(root).expect("create render context catalog");
        for (relative, source) in sources {
            write_test_file(&root.join(relative), source);
            catalog
                .upsert_from_source(relative, source.as_bytes())
                .expect("derive fixture render context");
        }
        catalog
    }

    fn commit_catalog(root: &Path, catalog: &RenderContextCatalog) {
        write_render_contexts_to(&render_contexts_path_in(root), catalog)
            .expect("commit render context catalog");
    }

    fn replace_context(root: &Path, relative: &str, source: &str) {
        write_test_file(&root.join(relative), source);
        let mut catalog = RenderContextCatalog::load_for_fixture_update(root, relative)
            .expect("load catalog for fixture update");
        catalog
            .upsert_from_source(relative, source.as_bytes())
            .expect("upsert fixture render context");
        commit_catalog(root, &catalog);
    }

    fn context_level(root: &Path, relative: &str) -> Option<SecurityLevel> {
        RenderContextCatalog::load(root)
            .expect("load render contexts")
            .context_for_relative_fixture(relative)
            .expect("look up relative fixture")
            .map(|context| context.security_level())
    }

    #[test]
    fn imported_fixture_config_look_detects_nested_yaml_frontmatter() {
        let look = imported_fixture_config_look(
            r#"---
config:
  look: handDrawn
---
flowchart TD
  A-->B
"#,
        );

        assert_eq!(look.as_deref(), Some("handDrawn"));
    }

    #[test]
    fn imported_fixture_config_look_detects_root_yaml_frontmatter() {
        let look = imported_fixture_config_look(
            r#"---
look: neo
---
flowchart TD
  A-->B
"#,
        );

        assert_eq!(look.as_deref(), Some("neo"));
    }

    #[test]
    fn committed_render_context_survives_backup_cleanup_failure() {
        let root = TestFixtureRoot::new();
        let path = render_contexts_path_in(root.path());
        let original =
            catalog_from_sources(root.path(), &[("flowchart/original.mmd", LOOSE_SOURCE)]);
        write_render_contexts_to(&path, &original).expect("write original render context");

        let updated =
            catalog_from_sources(root.path(), &[("sequence/updated.mmd", SANDBOX_SOURCE)]);
        write_render_contexts_to_with_backup_remover(&path, &updated, |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected backup cleanup failure",
            ))
        })
        .expect("backup cleanup is post-commit and must not fail the write");

        let committed = RenderContextCatalog::load(root.path()).expect("read committed contexts");
        assert_eq!(committed.contexts().count(), 1);
        assert_eq!(
            committed
                .context_for_relative_fixture("sequence/updated.mmd")
                .expect("look up updated context")
                .expect("updated context")
                .security_level(),
            SecurityLevel::Sandbox
        );
        let backup_count = fs::read_dir(path.parent().expect("render context parent"))
            .expect("read render context directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
            .count();
        assert_eq!(backup_count, 1, "failed cleanup may leave one backup");
    }

    #[test]
    fn imported_fixture_snapshot_restores_candidate_and_preserves_sibling_context() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let candidate_relative = "flowchart/candidate.mmd";
        let sibling_relative = "flowchart/sibling.mmd";
        let fixture_path = root.path().join(candidate_relative);
        let golden_path = golden_json_path_in(root.path(), diagram_dir, stem);
        let layout_path = layout_golden_json_path_in(root.path(), diagram_dir, stem);
        let deferred_path = deferred_fixture_path_in(root.path(), diagram_dir, stem);
        let deferred_svg_path = deferred_upstream_svg_path_in(root.path(), diagram_dir, stem);
        let upstream_path = upstream_svg_path_in(root.path(), diagram_dir, stem);
        let upstream_family = upstream_path.parent().expect("upstream family directory");
        let sibling_svg_path = upstream_family.join("sibling.svg");
        let baseline_manifest = upstream_family.join("_baseline-manifest.json");
        let failures_path = upstream_family.join("_failures.txt");

        let original_catalog = catalog_from_sources(
            root.path(),
            &[
                (candidate_relative, LOOSE_SOURCE),
                (sibling_relative, SANDBOX_SOURCE),
            ],
        );
        commit_catalog(root.path(), &original_catalog);
        write_test_file(&golden_path, b"old golden");
        write_test_file(&layout_path, b"old layout");
        write_test_file(&deferred_path, b"old deferred fixture");
        write_test_file(&deferred_svg_path, b"old deferred svg");
        write_test_file(&upstream_path, b"old upstream svg");
        write_test_file(&sibling_svg_path, b"old sibling svg");
        write_test_file(&baseline_manifest, b"old manifest");
        write_test_file(&failures_path, b"old failures");

        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture complete imported fixture state");

        write_test_file(&golden_path, b"new golden");
        write_test_file(&layout_path, b"new layout");
        write_test_file(&deferred_path, b"new deferred fixture");
        write_test_file(&deferred_svg_path, b"new deferred svg");
        write_test_file(&upstream_path, b"new upstream svg");
        write_test_file(&sibling_svg_path, b"new sibling svg");
        write_test_file(&baseline_manifest, b"new manifest");
        write_test_file(&failures_path, b"new failures");
        let added_svg_path = upstream_family.join("added.svg");
        write_test_file(&added_svg_path, b"added svg");
        let replacement_catalog = catalog_from_sources(
            root.path(),
            &[
                (candidate_relative, SANDBOX_SOURCE),
                (sibling_relative, LOOSE_SOURCE),
            ],
        );
        commit_catalog(root.path(), &replacement_catalog);

        assert!(snapshot.rollback().is_empty());
        assert_eq!(fs::read_to_string(&fixture_path).unwrap(), LOOSE_SOURCE);
        assert_eq!(fs::read(&golden_path).unwrap(), b"old golden");
        assert_eq!(fs::read(&layout_path).unwrap(), b"old layout");
        assert_eq!(fs::read(&deferred_path).unwrap(), b"old deferred fixture");
        assert_eq!(fs::read(&deferred_svg_path).unwrap(), b"old deferred svg");
        assert_eq!(fs::read(&upstream_path).unwrap(), b"old upstream svg");
        assert_eq!(fs::read(&sibling_svg_path).unwrap(), b"old sibling svg");
        assert_eq!(fs::read(&baseline_manifest).unwrap(), b"old manifest");
        assert_eq!(fs::read(&failures_path).unwrap(), b"old failures");
        assert!(!added_svg_path.exists());
        assert_eq!(
            context_level(root.path(), candidate_relative),
            Some(SecurityLevel::Loose)
        );
        assert_eq!(
            context_level(root.path(), sibling_relative),
            Some(SecurityLevel::Loose),
            "rollback must preserve the sibling's later committed context"
        );
    }

    #[test]
    fn imported_fixture_snapshot_can_preserve_new_deferred_files() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "sequence";
        let stem = "candidate";
        let relative = "sequence/candidate.mmd";
        let fixture_path = root.path().join(relative);
        let deferred_path = deferred_fixture_path_in(root.path(), diagram_dir, stem);
        let deferred_svg_path = deferred_upstream_svg_path_in(root.path(), diagram_dir, stem);
        let upstream_path = upstream_svg_path_in(root.path(), diagram_dir, stem);
        let baseline_manifest = upstream_path
            .parent()
            .expect("upstream family directory")
            .join("_baseline-manifest.json");
        let original = catalog_from_sources(root.path(), &[(relative, LOOSE_SOURCE)]);
        commit_catalog(root.path(), &original);
        write_test_file(&deferred_path, b"old deferred fixture");
        write_test_file(&deferred_svg_path, b"old deferred svg");
        write_test_file(&upstream_path, b"old upstream svg");
        write_test_file(&baseline_manifest, b"old manifest");

        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture imported fixture state");
        replace_context(root.path(), relative, SANDBOX_SOURCE);
        fs::remove_file(&fixture_path).expect("remove active fixture during simulated defer");
        write_test_file(&deferred_path, b"new deferred fixture");
        write_test_file(&deferred_svg_path, b"new deferred svg");
        write_test_file(&upstream_path, b"new upstream svg");
        write_test_file(&baseline_manifest, b"new manifest");

        assert!(snapshot.rollback_preserving_deferred().is_empty());
        assert_eq!(fs::read_to_string(&fixture_path).unwrap(), LOOSE_SOURCE);
        assert_eq!(fs::read(&deferred_path).unwrap(), b"new deferred fixture");
        assert_eq!(fs::read(&deferred_svg_path).unwrap(), b"new deferred svg");
        assert_eq!(fs::read(&upstream_path).unwrap(), b"old upstream svg");
        assert_eq!(fs::read(&baseline_manifest).unwrap(), b"old manifest");
        assert_eq!(
            context_level(root.path(), relative),
            Some(SecurityLevel::Loose)
        );
    }

    #[test]
    fn later_candidate_rollback_preserves_an_earlier_commit() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let accepted_stem = "accepted";
        let candidate_stem = "candidate";
        let accepted_relative = "flowchart/accepted.mmd";
        let candidate_relative = "flowchart/candidate.mmd";
        let accepted_fixture = root.path().join(accepted_relative);
        let candidate_fixture = root.path().join(candidate_relative);
        let accepted_svg = upstream_svg_path_in(root.path(), diagram_dir, accepted_stem);
        let candidate_svg = upstream_svg_path_in(root.path(), diagram_dir, candidate_stem);
        let baseline_manifest = accepted_svg
            .parent()
            .expect("upstream family directory")
            .join("_baseline-manifest.json");

        let accepted = catalog_from_sources(root.path(), &[(accepted_relative, LOOSE_SOURCE)]);
        commit_catalog(root.path(), &accepted);
        write_test_file(&accepted_svg, b"accepted svg");
        write_test_file(&baseline_manifest, b"accepted manifest");
        let candidate_snapshot = ImportedFixtureSnapshot::capture_in(
            root.path(),
            diagram_dir,
            candidate_stem,
            &candidate_fixture,
        )
        .expect("capture state after the earlier candidate committed");

        replace_context(root.path(), candidate_relative, SANDBOX_SOURCE);
        write_test_file(&accepted_svg, b"regenerated accepted svg");
        write_test_file(&candidate_svg, b"candidate svg");
        write_test_file(&baseline_manifest, b"candidate manifest");

        assert!(candidate_snapshot.rollback().is_empty());
        assert_eq!(fs::read_to_string(&accepted_fixture).unwrap(), LOOSE_SOURCE);
        assert_eq!(fs::read(&accepted_svg).unwrap(), b"accepted svg");
        assert_eq!(fs::read(&baseline_manifest).unwrap(), b"accepted manifest");
        assert!(!candidate_fixture.exists());
        assert!(!candidate_svg.exists());
        assert_eq!(
            context_level(root.path(), accepted_relative),
            Some(SecurityLevel::Loose)
        );
        assert_eq!(context_level(root.path(), candidate_relative), None);
    }

    #[test]
    fn render_context_rollback_restores_original_bytes_when_semantics_match() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let candidate_relative = "flowchart/candidate.mmd";
        let sibling_relative = "flowchart/sibling.mmd";
        let fixture_path = root.path().join(candidate_relative);
        let manifest_path = render_contexts_path_in(root.path());
        let catalog = catalog_from_sources(
            root.path(),
            &[
                (candidate_relative, LOOSE_SOURCE),
                (sibling_relative, SANDBOX_SOURCE),
            ],
        );
        let canonical = catalog.to_json().expect("render catalog");
        let value: serde_json::Value = serde_json::from_str(&canonical).expect("parse catalog");
        let original = format!(
            "{}\n",
            serde_json::to_string(&value).expect("compact catalog")
        );
        write_test_file(&manifest_path, original.as_bytes());
        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture non-canonical render context state");

        replace_context(root.path(), candidate_relative, SANDBOX_SOURCE);
        assert!(snapshot.rollback().is_empty());
        assert_eq!(fs::read_to_string(&manifest_path).unwrap(), original);
    }

    #[test]
    fn render_context_rollback_recovers_from_invalid_current_json() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let relative = "flowchart/candidate.mmd";
        let fixture_path = root.path().join(relative);
        let manifest_path = render_contexts_path_in(root.path());
        let catalog = catalog_from_sources(root.path(), &[(relative, LOOSE_SOURCE)]);
        commit_catalog(root.path(), &catalog);
        let original = fs::read(&manifest_path).expect("read original render contexts");
        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture imported fixture state");

        write_test_file(&manifest_path, b"{ truncated");

        assert!(snapshot.rollback().is_empty());
        assert_eq!(fs::read(&manifest_path).unwrap(), original);
    }

    #[test]
    fn deferring_without_a_baseline_removes_a_stale_deferred_svg() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let upstream_path = upstream_svg_path_in(root.path(), diagram_dir, stem);
        let deferred_svg_path = deferred_upstream_svg_path_in(root.path(), diagram_dir, stem);
        write_test_file(&fixture_path, b"new fixture");
        write_test_file(&upstream_path, b"old active svg");
        write_test_file(&deferred_svg_path, b"stale deferred svg");

        let deferred_path = defer_fixture_files_with_replace_existing_in(
            root.path(),
            diagram_dir,
            stem,
            &fixture_path,
            false,
            true,
        )
        .expect("defer fixture without an upstream baseline");

        assert_eq!(
            fs::read(&deferred_path).expect("read deferred fixture"),
            b"new fixture"
        );
        assert!(!upstream_path.exists());
        assert!(!deferred_svg_path.exists());
    }

    #[test]
    fn deferring_with_a_missing_baseline_restores_the_previous_pair() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let deferred_path = deferred_fixture_path_in(root.path(), diagram_dir, stem);
        let deferred_svg_path = deferred_upstream_svg_path_in(root.path(), diagram_dir, stem);
        write_test_file(&fixture_path, b"new active fixture");
        write_test_file(&deferred_path, b"old deferred fixture");
        write_test_file(&deferred_svg_path, b"old deferred svg");

        defer_fixture_files_with_replace_existing_in(
            root.path(),
            diagram_dir,
            stem,
            &fixture_path,
            true,
            true,
        )
        .expect_err("a missing canonical baseline must fail");

        assert_eq!(
            fs::read(&fixture_path).expect("read restored active fixture"),
            b"new active fixture"
        );
        assert_eq!(
            fs::read(&deferred_path).expect("read restored deferred fixture"),
            b"old deferred fixture"
        );
        assert_eq!(
            fs::read(&deferred_svg_path).expect("read restored deferred svg"),
            b"old deferred svg"
        );
    }

    #[test]
    fn defer_failure_restores_files_moved_before_the_error() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let upstream_path = upstream_svg_path_in(root.path(), diagram_dir, stem);
        let blocker_path = root.path().join("_deferred").join("upstream-svgs");
        write_test_file(&fixture_path, b"active fixture");
        write_test_file(&upstream_path, b"active upstream svg");
        write_test_file(&blocker_path, b"not a directory");

        let error = defer_fixture_files_with_replace_existing_in(
            root.path(),
            diagram_dir,
            stem,
            &fixture_path,
            true,
            true,
        )
        .expect_err("blocked deferred SVG directory must fail");

        assert!(error.to_string().contains("upstream-svgs"));
        assert_eq!(
            fs::read(&fixture_path).expect("read restored active fixture"),
            b"active fixture"
        );
        assert_eq!(
            fs::read(&upstream_path).expect("read restored active upstream svg"),
            b"active upstream svg"
        );
        assert!(!deferred_fixture_path_in(root.path(), diagram_dir, stem).exists());
        assert_eq!(
            fs::read(&blocker_path).expect("read deferred directory blocker"),
            b"not a directory"
        );
    }

    #[test]
    fn defer_rejects_a_directory_destination_without_touching_the_source() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let deferred_path = deferred_fixture_path_in(root.path(), diagram_dir, stem);
        write_test_file(&fixture_path, b"active fixture");
        fs::create_dir_all(&deferred_path).expect("create directory at deferred fixture path");

        defer_fixture_files_with_replace_existing_in(
            root.path(),
            diagram_dir,
            stem,
            &fixture_path,
            false,
            true,
        )
        .expect_err("directory destination must fail");

        assert_eq!(
            fs::read(&fixture_path).expect("read untouched active fixture"),
            b"active fixture"
        );
        assert!(deferred_path.is_dir());
    }
}
