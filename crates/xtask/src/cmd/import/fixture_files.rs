use crate::XtaskError;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SITE_CONFIG_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

fn site_config_overrides_path_in(root: &Path) -> PathBuf {
    root.join("_config").join("site_config_overrides.json")
}

fn site_config_overrides_path() -> PathBuf {
    site_config_overrides_path_in(&fixtures_root())
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
    site_config_file: ImportedFileSnapshot,
    site_config_overrides: serde_json::Map<String, serde_json::Value>,
    site_config_relative_path: String,
    site_config_value: Option<serde_json::Value>,
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

        let site_config_path = site_config_overrides_path_in(root);
        let site_config_file = ImportedFileSnapshot::capture(site_config_path.clone())?;
        let site_config_relative_path = fixture_relative_path(diagram_dir, stem);
        let site_config_overrides = read_site_config_overrides_from(&site_config_path)?;
        let site_config_value = site_config_overrides
            .get(&site_config_relative_path)
            .cloned();

        Ok(Self {
            active_files,
            deferred_files,
            upstream_family,
            site_config_file,
            site_config_overrides,
            site_config_relative_path,
            site_config_value,
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
        if let Err(err) = self.rollback_site_config_override() {
            errors.push(err);
        }
        errors
    }

    fn rollback_site_config_override(&self) -> Result<(), String> {
        let mut overrides = match read_site_config_overrides_from(&self.site_config_file.path) {
            Ok(overrides) => overrides,
            Err(_) => return self.site_config_file.rollback(),
        };
        let candidate_changed =
            overrides.get(&self.site_config_relative_path) != self.site_config_value.as_ref();
        if candidate_changed {
            match &self.site_config_value {
                Some(value) => {
                    overrides.insert(self.site_config_relative_path.clone(), value.clone());
                }
                None => {
                    overrides.remove(&self.site_config_relative_path);
                }
            }
        }

        if json_object_semantically_equal(&overrides, &self.site_config_overrides) {
            return self.site_config_file.rollback();
        }
        if !candidate_changed {
            return Ok(());
        }

        if self.site_config_file.contents.is_none() && overrides.is_empty() {
            return match fs::remove_file(&self.site_config_file.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(format!(
                    "failed to remove fixture site config manifest {} during rollback: {err}",
                    self.site_config_file.path.display()
                )),
            };
        }

        write_site_config_overrides_to(&self.site_config_file.path, overrides)
            .map_err(|err| err.to_string())
    }
}

fn json_object_semantically_equal(
    left: &serde_json::Map<String, serde_json::Value>,
    right: &serde_json::Map<String, serde_json::Value>,
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(key, left_value)| {
            right
                .get(key)
                .is_some_and(|right_value| json_value_semantically_equal(left_value, right_value))
        })
}

fn json_value_semantically_equal(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    match (left, right) {
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| json_value_semantically_equal(left, right))
        }
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            json_object_semantically_equal(left, right)
        }
        _ => left == right,
    }
}

fn normalize_security_level(value: &str) -> Option<&'static str> {
    match value.trim() {
        "loose" => Some("loose"),
        "sandbox" => Some("sandbox"),
        _ => None,
    }
}

fn security_level_from_json(value: &serde_json::Value) -> Option<&'static str> {
    if let Some(level) = value
        .get("securityLevel")
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_security_level)
    {
        return Some(level);
    }

    value
        .get("config")
        .and_then(|config| config.get("securityLevel"))
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_security_level)
}

fn security_level_from_yaml(value: &serde_json::Value) -> Option<&'static str> {
    let mapping = value.as_object()?;
    let direct = "securityLevel";
    if let Some(level) = mapping
        .get(direct)
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_security_level)
    {
        return Some(level);
    }

    let config_key = "config";
    mapping
        .get(config_key)
        .and_then(serde_json::Value::as_object)
        .and_then(|config| config.get(direct))
        .and_then(serde_json::Value::as_str)
        .and_then(normalize_security_level)
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

fn security_level_from_frontmatter(body: &str) -> Option<&'static str> {
    let (yaml, _) = split_yaml_frontmatter(body)?;
    let parsed = serde_saphyr::from_str::<serde_json::Value>(yaml).ok()?;
    security_level_from_yaml(&parsed)
}

pub(crate) fn imported_fixture_config_look(body: &str) -> Option<String> {
    let (yaml, _) = split_yaml_frontmatter(body)?;
    let parsed = serde_saphyr::from_str::<serde_json::Value>(yaml).ok()?;
    config_look_from_yaml(&parsed).map(str::to_string)
}

fn security_level_from_directives(body: &str) -> Option<&'static str> {
    let mut start = 0usize;
    while let Some(rel_start) = body[start..].find("%%{") {
        let content_start = start + rel_start + 3;
        let Some(rel_end) = body[content_start..].find("}%%") else {
            break;
        };
        let content_end = content_start + rel_end;
        let raw = body[content_start..content_end].trim();
        start = content_end + 3;

        let Some((directive, args)) = raw.split_once(':') else {
            continue;
        };
        let directive = directive.trim();
        if directive != "init" && directive != "initialize" {
            continue;
        }

        let args = args.trim().replace('\'', "\"");
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&args) else {
            continue;
        };
        if let Some(level) = security_level_from_json(&value) {
            return Some(level);
        }
    }
    None
}

fn imported_fixture_site_config(body: &str) -> Option<serde_json::Value> {
    let security_level =
        security_level_from_frontmatter(body).or_else(|| security_level_from_directives(body))?;

    Some(serde_json::json!({
        "securityLevel": security_level
    }))
}

fn read_site_config_overrides() -> Result<serde_json::Map<String, serde_json::Value>, XtaskError> {
    let path = site_config_overrides_path();
    read_site_config_overrides_from(&path)
}

fn read_site_config_overrides_from(
    path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>, XtaskError> {
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }

    let text = fs::read_to_string(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))?;
    match value {
        serde_json::Value::Object(map) => Ok(map),
        other => Err(XtaskError::SnapshotUpdateFailed(format!(
            "fixture site config override manifest must be a JSON object, got {other:?}"
        ))),
    }
}

fn write_site_config_overrides(
    overrides: serde_json::Map<String, serde_json::Value>,
) -> Result<(), XtaskError> {
    let path = site_config_overrides_path();
    write_site_config_overrides_to(&path, overrides)
}

fn write_site_config_overrides_to(
    path: &Path,
    overrides: serde_json::Map<String, serde_json::Value>,
) -> Result<(), XtaskError> {
    write_site_config_overrides_to_with_backup_remover(path, overrides, |backup_path| {
        fs::remove_file(backup_path)
    })
}

fn write_site_config_overrides_to_with_backup_remover<R>(
    path: &Path,
    overrides: serde_json::Map<String, serde_json::Value>,
    remove_backup: R,
) -> Result<(), XtaskError>
where
    R: FnOnce(&Path) -> std::io::Result<()>,
{
    let pretty = render_site_config_overrides(&overrides)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }

    let sequence = SITE_CONFIG_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("site_config_overrides.json");
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
                source: std::io::Error::other("site config manifest path is not a file"),
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
                    "failed to install site config manifest: {source}; failed to restore backup: {rollback_error}"
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
            "warning: failed to remove committed fixture site config backup {}: {err}",
            backup_path.display()
        );
    }
    Ok(())
}

fn render_site_config_overrides(
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<String, XtaskError> {
    let mut out = String::from("{\n");
    for (idx, (key, value)) in overrides.iter().enumerate() {
        if idx > 0 {
            out.push_str(",\n");
        }
        let key = serde_json::to_string(key)
            .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))?;
        let value = render_json_inline(value)?;
        out.push_str("  ");
        out.push_str(&key);
        out.push_str(": ");
        out.push_str(&value);
    }
    out.push_str("\n}\n");
    Ok(out)
}

fn render_json_inline(value: &serde_json::Value) -> Result<String, XtaskError> {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            serde_json::to_string(value)
                .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))
        }
        serde_json::Value::String(value) => serde_json::to_string(value)
            .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string())),
        serde_json::Value::Array(items) => {
            let rendered = items
                .iter()
                .map(render_json_inline)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("[{}]", rendered.join(", ")))
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                return Ok("{}".to_string());
            }
            let mut rendered = Vec::with_capacity(map.len());
            for (key, value) in map {
                let key = serde_json::to_string(key)
                    .map_err(|err| XtaskError::SnapshotUpdateFailed(err.to_string()))?;
                rendered.push(format!("{key}: {}", render_json_inline(value)?));
            }
            Ok(format!("{{ {} }}", rendered.join(", ")))
        }
    }
}

fn apply_site_config_override(
    overrides: &mut serde_json::Map<String, serde_json::Value>,
    relative_path: String,
    body: &str,
) -> bool {
    match imported_fixture_site_config(body) {
        Some(site_config) => {
            if overrides.get(&relative_path) == Some(&site_config) {
                false
            } else {
                overrides.insert(relative_path, site_config);
                true
            }
        }
        None => overrides.remove(&relative_path).is_some(),
    }
}

fn update_site_config_override(
    diagram_dir: &str,
    stem: &str,
    body: &str,
) -> Result<(), XtaskError> {
    let relative_path = fixture_relative_path(diagram_dir, stem);
    let mut overrides = read_site_config_overrides()?;
    if apply_site_config_override(&mut overrides, relative_path, body) {
        write_site_config_overrides(overrides)?;
    }
    Ok(())
}

fn remove_site_config_override(diagram_dir: &str, stem: &str) -> Result<(), XtaskError> {
    let path = site_config_overrides_path();
    remove_site_config_override_from(&path, diagram_dir, stem)
}

fn remove_site_config_override_from(
    path: &Path,
    diagram_dir: &str,
    stem: &str,
) -> Result<(), XtaskError> {
    let mut overrides = read_site_config_overrides_from(path)?;
    if overrides
        .remove(&fixture_relative_path(diagram_dir, stem))
        .is_some()
    {
        write_site_config_overrides_to(path, overrides)?;
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
        remove_site_config_override(diagram_dir, stem)
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
    update_site_config_override(diagram_dir, stem, body)
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
        remove_site_config_override_from(&site_config_overrides_path_in(root), diagram_dir, stem)
    })();
    result
        .map(|()| deferred_path)
        .map_err(|error| rollback_failed_file_operation(error, &snapshot))
}

#[cfg(test)]
mod tests {
    use super::{
        ImportedFixtureSnapshot, apply_site_config_override,
        defer_fixture_files_with_replace_existing_in, deferred_fixture_path_in,
        deferred_upstream_svg_path_in, golden_json_path_in, imported_fixture_config_look,
        imported_fixture_site_config, layout_golden_json_path_in, read_site_config_overrides_from,
        render_site_config_overrides, site_config_overrides_path_in, upstream_svg_path_in,
        write_site_config_overrides_to, write_site_config_overrides_to_with_backup_remover,
    };
    use serde_json::json;
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

    #[test]
    fn imported_fixture_site_config_detects_loose_json_directive() {
        let site_config = imported_fixture_site_config(
            r#"%%{init: {"securityLevel":"loose","theme":"dark"}}%%
flowchart TD
  A-->B
"#,
        )
        .expect("loose security level should become site config");

        assert_eq!(site_config["securityLevel"], "loose");
    }

    #[test]
    fn imported_fixture_site_config_detects_single_quote_directive() {
        let site_config = imported_fixture_site_config(
            r#"%%{init: {'securityLevel':'sandbox'}}%%
flowchart TD
  A-->B
"#,
        )
        .expect("single-quoted directive should become site config");

        assert_eq!(site_config["securityLevel"], "sandbox");
    }

    #[test]
    fn imported_fixture_site_config_detects_nested_config_directive() {
        let site_config = imported_fixture_site_config(
            r#"%%{init: {"config": {"securityLevel": "loose"}}}%%
flowchart TD
  A-->B
"#,
        )
        .expect("nested config directive should become site config");

        assert_eq!(site_config["securityLevel"], "loose");
    }

    #[test]
    fn imported_fixture_site_config_detects_sandbox_yaml_frontmatter() {
        let site_config = imported_fixture_site_config(
            r#"---
config:
  securityLevel: sandbox
---
flowchart TD
  A-->B
"#,
        )
        .expect("sandbox security level should become site config");

        assert_eq!(site_config["securityLevel"], "sandbox");
    }

    #[test]
    fn imported_fixture_site_config_detects_root_yaml_frontmatter() {
        let site_config = imported_fixture_site_config(
            r#"---
securityLevel: loose
---
flowchart TD
  A-->B
"#,
        )
        .expect("root frontmatter security level should become site config");

        assert_eq!(site_config["securityLevel"], "loose");
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
    fn render_site_config_overrides_keeps_compact_entry_lines() {
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "flowchart/a.mmd".to_string(),
            json!({ "securityLevel": "loose" }),
        );
        overrides.insert(
            "flowchart/b.mmd".to_string(),
            json!({ "securityLevel": "sandbox" }),
        );

        let rendered = render_site_config_overrides(&overrides).expect("render overrides");

        assert_eq!(
            rendered,
            "{\n  \"flowchart/a.mmd\": { \"securityLevel\": \"loose\" },\n  \"flowchart/b.mmd\": { \"securityLevel\": \"sandbox\" }\n}\n"
        );
    }

    #[test]
    fn imported_fixture_site_config_ignores_strict_default() {
        assert!(
            imported_fixture_site_config(
                r#"%%{init: {"securityLevel":"strict"}}%%
flowchart TD
  A-->B
"#
            )
            .is_none()
        );
    }

    #[test]
    fn imported_fixture_site_config_ignores_label_text() {
        assert!(
            imported_fixture_site_config(
                r#"flowchart TD
  A["securityLevel: loose"]
"#
            )
            .is_none()
        );
    }

    #[test]
    fn apply_site_config_override_adds_detected_security_level() {
        let mut overrides = serde_json::Map::new();

        let changed = apply_site_config_override(
            &mut overrides,
            "flowchart/example.mmd".to_string(),
            r#"%%{init: {"securityLevel":"loose"}}%%
flowchart TD
  A-->B
"#,
        );

        assert!(changed);
        assert_eq!(overrides["flowchart/example.mmd"]["securityLevel"], "loose");
    }

    #[test]
    fn apply_site_config_override_removes_stale_entry_for_default_fixture() {
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "flowchart/example.mmd".to_string(),
            json!({ "securityLevel": "loose" }),
        );

        let changed = apply_site_config_override(
            &mut overrides,
            "flowchart/example.mmd".to_string(),
            "flowchart TD\n  A-->B\n",
        );

        assert!(changed);
        assert!(overrides.get("flowchart/example.mmd").is_none());
    }

    #[test]
    fn apply_site_config_override_skips_unchanged_manifest() {
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "flowchart/example.mmd".to_string(),
            json!({ "securityLevel": "sandbox" }),
        );

        let changed = apply_site_config_override(
            &mut overrides,
            "flowchart/example.mmd".to_string(),
            r#"%%{init: {"securityLevel":"sandbox"}}%%
flowchart TD
  A-->B
"#,
        );

        assert!(!changed);
        assert_eq!(
            overrides["flowchart/example.mmd"]["securityLevel"],
            "sandbox"
        );
    }

    #[test]
    fn committed_site_config_survives_backup_cleanup_failure() {
        let root = TestFixtureRoot::new();
        let path = site_config_overrides_path_in(root.path());
        write_site_config_overrides_to(
            &path,
            serde_json::Map::from_iter([(
                "flowchart/original.mmd".to_string(),
                json!({ "securityLevel": "loose" }),
            )]),
        )
        .expect("write original site config");

        let updated = serde_json::Map::from_iter([(
            "sequence/updated.mmd".to_string(),
            json!({ "securityLevel": "sandbox" }),
        )]);
        write_site_config_overrides_to_with_backup_remover(&path, updated.clone(), |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected backup cleanup failure",
            ))
        })
        .expect("backup cleanup is post-commit and must not fail the write");

        assert_eq!(
            read_site_config_overrides_from(&path).expect("read committed site config"),
            updated
        );
        let backup_count = fs::read_dir(path.parent().expect("site config parent"))
            .expect("read site config directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".backup"))
            .count();
        assert_eq!(backup_count, 1, "failed cleanup may leave one backup");
    }

    #[test]
    fn imported_fixture_snapshot_restores_all_managed_candidate_state() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let golden_path = golden_json_path_in(root.path(), diagram_dir, stem);
        let layout_path = layout_golden_json_path_in(root.path(), diagram_dir, stem);
        let deferred_path = deferred_fixture_path_in(root.path(), diagram_dir, stem);
        let deferred_svg_path = deferred_upstream_svg_path_in(root.path(), diagram_dir, stem);
        let upstream_path = upstream_svg_path_in(root.path(), diagram_dir, stem);
        let upstream_family = upstream_path.parent().expect("upstream family directory");
        let sibling_svg_path = upstream_family.join("sibling.svg");
        let manifest_path = upstream_family.join("_baseline-manifest.json");
        let failures_path = upstream_family.join("_failures.txt");
        let site_config_path = site_config_overrides_path_in(root.path());

        write_test_file(&fixture_path, b"old fixture");
        write_test_file(&golden_path, b"old golden");
        write_test_file(&layout_path, b"old layout");
        write_test_file(&deferred_path, b"old deferred fixture");
        write_test_file(&deferred_svg_path, b"old deferred svg");
        write_test_file(&upstream_path, b"old upstream svg");
        write_test_file(&sibling_svg_path, b"old sibling svg");
        write_test_file(&manifest_path, b"old manifest");
        write_test_file(&failures_path, b"old failures");
        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([
                (
                    "flowchart/candidate.mmd".to_string(),
                    json!({ "securityLevel": "loose" }),
                ),
                (
                    "flowchart/sibling.mmd".to_string(),
                    json!({ "securityLevel": "sandbox" }),
                ),
            ]),
        )
        .expect("write original site config overrides");

        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture complete imported fixture state");

        write_test_file(&fixture_path, b"new fixture");
        write_test_file(&golden_path, b"new golden");
        write_test_file(&layout_path, b"new layout");
        write_test_file(&deferred_path, b"new deferred fixture");
        write_test_file(&deferred_svg_path, b"new deferred svg");
        write_test_file(&upstream_path, b"new upstream svg");
        write_test_file(&sibling_svg_path, b"new sibling svg");
        write_test_file(&manifest_path, b"new manifest");
        write_test_file(&failures_path, b"new failures");
        let added_svg_path = upstream_family.join("added.svg");
        write_test_file(&added_svg_path, b"added svg");
        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([
                (
                    "flowchart/candidate.mmd".to_string(),
                    json!({ "securityLevel": "sandbox" }),
                ),
                (
                    "flowchart/sibling.mmd".to_string(),
                    json!({ "securityLevel": "loose" }),
                ),
            ]),
        )
        .expect("write replacement site config overrides");

        assert!(snapshot.rollback().is_empty());
        assert_eq!(
            fs::read(&fixture_path).expect("read fixture"),
            b"old fixture"
        );
        assert_eq!(fs::read(&golden_path).expect("read golden"), b"old golden");
        assert_eq!(fs::read(&layout_path).expect("read layout"), b"old layout");
        assert_eq!(
            fs::read(&deferred_path).expect("read deferred fixture"),
            b"old deferred fixture"
        );
        assert_eq!(
            fs::read(&deferred_svg_path).expect("read deferred svg"),
            b"old deferred svg"
        );
        assert_eq!(
            fs::read(&upstream_path).expect("read upstream svg"),
            b"old upstream svg"
        );
        assert_eq!(
            fs::read(&sibling_svg_path).expect("read sibling svg"),
            b"old sibling svg"
        );
        assert_eq!(
            fs::read(&manifest_path).expect("read manifest"),
            b"old manifest"
        );
        assert_eq!(
            fs::read(&failures_path).expect("read failures"),
            b"old failures"
        );
        assert!(!added_svg_path.exists());

        let overrides =
            read_site_config_overrides_from(&site_config_path).expect("read restored site config");
        assert_eq!(
            overrides["flowchart/candidate.mmd"]["securityLevel"],
            "loose"
        );
        assert_eq!(overrides["flowchart/sibling.mmd"]["securityLevel"], "loose");
    }

    #[test]
    fn imported_fixture_snapshot_can_preserve_new_deferred_files() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "sequence";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let deferred_path = deferred_fixture_path_in(root.path(), diagram_dir, stem);
        let deferred_svg_path = deferred_upstream_svg_path_in(root.path(), diagram_dir, stem);
        let upstream_path = upstream_svg_path_in(root.path(), diagram_dir, stem);
        let manifest_path = upstream_path
            .parent()
            .expect("upstream family directory")
            .join("_baseline-manifest.json");
        let site_config_path = site_config_overrides_path_in(root.path());

        write_test_file(&fixture_path, b"old active fixture");
        write_test_file(&deferred_path, b"old deferred fixture");
        write_test_file(&deferred_svg_path, b"old deferred svg");
        write_test_file(&upstream_path, b"old upstream svg");
        write_test_file(&manifest_path, b"old manifest");
        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([(
                "sequence/candidate.mmd".to_string(),
                json!({ "securityLevel": "loose" }),
            )]),
        )
        .expect("write original site config overrides");

        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture imported fixture state");
        fs::remove_file(&fixture_path).expect("remove active fixture during simulated defer");
        write_test_file(&deferred_path, b"new deferred fixture");
        write_test_file(&deferred_svg_path, b"new deferred svg");
        write_test_file(&upstream_path, b"new upstream svg");
        write_test_file(&manifest_path, b"new manifest");
        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([(
                "sequence/candidate.mmd".to_string(),
                json!({ "securityLevel": "sandbox" }),
            )]),
        )
        .expect("write replacement site config overrides");

        assert!(snapshot.rollback_preserving_deferred().is_empty());
        assert_eq!(
            fs::read(&fixture_path).expect("read active fixture"),
            b"old active fixture"
        );
        assert_eq!(
            fs::read(&deferred_path).expect("read deferred fixture"),
            b"new deferred fixture"
        );
        assert_eq!(
            fs::read(&deferred_svg_path).expect("read deferred svg"),
            b"new deferred svg"
        );
        assert_eq!(
            fs::read(&upstream_path).expect("read upstream svg"),
            b"old upstream svg"
        );
        assert_eq!(
            fs::read(&manifest_path).expect("read manifest"),
            b"old manifest"
        );
        let overrides =
            read_site_config_overrides_from(&site_config_path).expect("read restored site config");
        assert_eq!(
            overrides["sequence/candidate.mmd"]["securityLevel"],
            "loose"
        );
    }

    #[test]
    fn later_candidate_rollback_preserves_an_earlier_commit() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let accepted_stem = "accepted";
        let candidate_stem = "candidate";
        let accepted_fixture = root
            .path()
            .join(diagram_dir)
            .join(format!("{accepted_stem}.mmd"));
        let candidate_fixture = root
            .path()
            .join(diagram_dir)
            .join(format!("{candidate_stem}.mmd"));
        let accepted_svg = upstream_svg_path_in(root.path(), diagram_dir, accepted_stem);
        let candidate_svg = upstream_svg_path_in(root.path(), diagram_dir, candidate_stem);
        let manifest_path = accepted_svg
            .parent()
            .expect("upstream family directory")
            .join("_baseline-manifest.json");
        let site_config_path = site_config_overrides_path_in(root.path());

        write_test_file(&accepted_fixture, b"accepted fixture");
        write_test_file(&accepted_svg, b"accepted svg");
        write_test_file(&manifest_path, b"accepted manifest");
        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([(
                "flowchart/accepted.mmd".to_string(),
                json!({ "securityLevel": "loose" }),
            )]),
        )
        .expect("write accepted site config state");

        let candidate_snapshot = ImportedFixtureSnapshot::capture_in(
            root.path(),
            diagram_dir,
            candidate_stem,
            &candidate_fixture,
        )
        .expect("capture state after the earlier candidate committed");

        write_test_file(&candidate_fixture, b"candidate fixture");
        write_test_file(&accepted_svg, b"regenerated accepted svg");
        write_test_file(&candidate_svg, b"candidate svg");
        write_test_file(&manifest_path, b"candidate manifest");
        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([
                (
                    "flowchart/accepted.mmd".to_string(),
                    json!({ "securityLevel": "loose" }),
                ),
                (
                    "flowchart/candidate.mmd".to_string(),
                    json!({ "securityLevel": "sandbox" }),
                ),
            ]),
        )
        .expect("write candidate site config state");

        assert!(candidate_snapshot.rollback().is_empty());
        assert_eq!(
            fs::read(&accepted_fixture).expect("read accepted fixture"),
            b"accepted fixture"
        );
        assert_eq!(
            fs::read(&accepted_svg).expect("read accepted svg"),
            b"accepted svg"
        );
        assert_eq!(
            fs::read(&manifest_path).expect("read accepted manifest"),
            b"accepted manifest"
        );
        assert!(!candidate_fixture.exists());
        assert!(!candidate_svg.exists());
        let overrides =
            read_site_config_overrides_from(&site_config_path).expect("read accepted site config");
        assert_eq!(
            overrides["flowchart/accepted.mmd"]["securityLevel"],
            "loose"
        );
        assert!(overrides.get("flowchart/candidate.mmd").is_none());
    }

    #[test]
    fn site_config_rollback_restores_original_bytes_when_semantics_match_snapshot() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let site_config_path = site_config_overrides_path_in(root.path());
        write_test_file(&fixture_path, b"fixture");
        let original = br#"{
    "flowchart/sibling.mmd": {
        "securityLevel": "sandbox"
    },
    "flowchart/candidate.mmd" : { "securityLevel" : "loose" }
}
"#;
        write_test_file(&site_config_path, original);
        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture non-canonical site config state");

        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([
                (
                    "flowchart/candidate.mmd".to_string(),
                    json!({ "securityLevel": "sandbox" }),
                ),
                (
                    "flowchart/sibling.mmd".to_string(),
                    json!({ "securityLevel": "sandbox" }),
                ),
            ]),
        )
        .expect("write canonical replacement site config state");

        assert!(snapshot.rollback().is_empty());
        assert_eq!(
            fs::read(&site_config_path).expect("read byte-exact restored site config"),
            original
        );
    }

    #[test]
    fn site_config_rollback_recovers_from_invalid_current_json() {
        let root = TestFixtureRoot::new();
        let diagram_dir = "flowchart";
        let stem = "candidate";
        let fixture_path = root.path().join(diagram_dir).join(format!("{stem}.mmd"));
        let site_config_path = site_config_overrides_path_in(root.path());
        write_test_file(&fixture_path, b"fixture");
        write_site_config_overrides_to(
            &site_config_path,
            serde_json::Map::from_iter([(
                "flowchart/candidate.mmd".to_string(),
                json!({ "securityLevel": "loose" }),
            )]),
        )
        .expect("write original site config state");
        let original = fs::read(&site_config_path).expect("read original site config bytes");
        let snapshot =
            ImportedFixtureSnapshot::capture_in(root.path(), diagram_dir, stem, &fixture_path)
                .expect("capture imported fixture state");

        write_test_file(&site_config_path, b"{ truncated");

        assert!(snapshot.rollback().is_empty());
        assert_eq!(
            fs::read(&site_config_path).expect("read restored site config bytes"),
            original
        );
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
