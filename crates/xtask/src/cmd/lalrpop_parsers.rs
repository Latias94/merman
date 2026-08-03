//! Checked-in LALRPOP parser generation and byte-for-byte freshness verification.

use crate::XtaskError;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const CORE_SOURCE_DIR: &str = "crates/merman-core/src";
const CHECKED_IN_DIR: &str = "crates/merman-core/src/generated/lalrpop";
const REGENERATE_COMMAND: &str = "cargo run -p xtask -- gen-lalrpop-parsers";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GrammarArtifact {
    source: &'static str,
    generated: &'static str,
}

const GRAMMAR_ARTIFACTS: [GrammarArtifact; 5] = [
    GrammarArtifact {
        source: "diagrams/class_grammar.lalrpop",
        generated: "class_grammar.rs",
    },
    GrammarArtifact {
        source: "diagrams/er_grammar.lalrpop",
        generated: "er_grammar.rs",
    },
    GrammarArtifact {
        source: "diagrams/flowchart_grammar.lalrpop",
        generated: "flowchart_grammar.rs",
    },
    GrammarArtifact {
        source: "diagrams/sequence_grammar.lalrpop",
        generated: "sequence_grammar.rs",
    },
    GrammarArtifact {
        source: "diagrams/state_grammar.lalrpop",
        generated: "state_grammar.rs",
    },
];

type GeneratedParserSet = BTreeMap<String, Vec<u8>>;

fn parser_error(message: impl Into<String>) -> XtaskError {
    XtaskError::LalrpopParsers(message.into())
}

/// LALRPOP 0.23.1 emits a few spaces immediately before newlines. Keep checked-in generated
/// Rust compatible with the repository's whitespace gate without changing parser semantics.
fn canonicalize_generated_parser_bytes(bytes: Vec<u8>) -> Vec<u8> {
    let mut canonical = Vec::with_capacity(bytes.len());
    let mut line_start = 0;

    for line_end in 0..=bytes.len() {
        if line_end != bytes.len() && bytes[line_end] != b'\n' {
            continue;
        }

        let mut content_end = line_end;
        let has_carriage_return = content_end > line_start && bytes[content_end - 1] == b'\r';
        if has_carriage_return {
            content_end -= 1;
        }
        while content_end > line_start && matches!(bytes[content_end - 1], b' ' | b'\t') {
            content_end -= 1;
        }
        canonical.extend_from_slice(&bytes[line_start..content_end]);
        if has_carriage_return {
            canonical.push(b'\r');
        }
        if line_end != bytes.len() {
            canonical.push(b'\n');
        }
        line_start = line_end.saturating_add(1);
    }

    canonical
}

fn normalize_grammar_source_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' {
            normalized.push(b'\n');
            index += usize::from(bytes.get(index + 1) == Some(&b'\n')) + 1;
        } else {
            normalized.push(bytes[index]);
            index += 1;
        }
    }
    normalized
}

fn stage_normalized_grammar_sources(source_root: &Path) -> Result<tempfile::TempDir, XtaskError> {
    let staged = tempfile::tempdir().map_err(|source| XtaskError::WriteFile {
        path: std::env::temp_dir().display().to_string(),
        source,
    })?;
    for artifact in GRAMMAR_ARTIFACTS {
        let relative_path = PathBuf::from(artifact.source);
        let source_path = source_root.join(&relative_path);
        let destination_path = staged.path().join(&relative_path);
        let bytes = fs::read(&source_path).map_err(|source| XtaskError::ReadFile {
            path: source_path.display().to_string(),
            source,
        })?;
        let parent = destination_path.parent().ok_or_else(|| {
            parser_error(format!(
                "grammar destination has no parent: {}",
                destination_path.display()
            ))
        })?;
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
        fs::write(&destination_path, normalize_grammar_source_bytes(&bytes)).map_err(|source| {
            XtaskError::WriteFile {
                path: destination_path.display().to_string(),
                source,
            }
        })?;
    }
    Ok(staged)
}

fn read_directory_files(root: &Path, extension: &str) -> Result<BTreeSet<PathBuf>, XtaskError> {
    fn visit(
        root: &Path,
        current: &Path,
        extension: &str,
        files: &mut BTreeSet<PathBuf>,
    ) -> Result<(), XtaskError> {
        let entries = fs::read_dir(current).map_err(|source| XtaskError::ReadFile {
            path: current.display().to_string(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| XtaskError::ReadFile {
                path: current.display().to_string(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| XtaskError::ReadFile {
                path: path.display().to_string(),
                source,
            })?;
            if file_type.is_symlink() {
                return Err(parser_error(format!(
                    "refusing symlink while scanning parser inputs or outputs: {}",
                    path.display()
                )));
            }
            if file_type.is_dir() {
                visit(root, &path, extension, files)?;
            } else if file_type.is_file()
                && path.extension().and_then(|value| value.to_str()) == Some(extension)
            {
                files.insert(
                    path.strip_prefix(root)
                        .map_err(|_| {
                            parser_error(format!(
                                "scanned path escaped its root: {}",
                                path.display()
                            ))
                        })?
                        .to_path_buf(),
                );
            }
        }
        Ok(())
    }

    let mut files = BTreeSet::new();
    visit(root, root, extension, &mut files)?;
    Ok(files)
}

fn validate_grammar_source_set(source_root: &Path) -> Result<(), XtaskError> {
    let expected = GRAMMAR_ARTIFACTS
        .iter()
        .map(|artifact| PathBuf::from(artifact.source))
        .collect::<BTreeSet<_>>();
    let actual = read_directory_files(source_root, "lalrpop")?;
    if actual != expected {
        return Err(parser_error(format!(
            "the LALRPOP grammar source set is not the declared five-file set; expected [{}], found [{}]",
            display_paths(&expected),
            display_paths(&actual)
        )));
    }
    Ok(())
}

fn display_paths(paths: &BTreeSet<PathBuf>) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn generate_parser_set(root: &Path) -> Result<GeneratedParserSet, XtaskError> {
    let source_root = root.join(CORE_SOURCE_DIR);
    validate_grammar_source_set(&source_root)?;
    let normalized_sources = stage_normalized_grammar_sources(&source_root)?;

    let generated = tempfile::tempdir().map_err(|source| XtaskError::WriteFile {
        path: std::env::temp_dir().display().to_string(),
        source,
    })?;
    lalrpop::Configuration::new()
        .set_in_dir(normalized_sources.path())
        .set_out_dir(generated.path())
        .set_features(std::iter::empty::<String>())
        .force_build(true)
        .emit_rerun_directives(false)
        .log_quiet()
        .process()
        .map_err(|error| parser_error(format!("failed to generate parsers: {error}")))?;

    let expected_outputs = GRAMMAR_ARTIFACTS
        .iter()
        .map(|artifact| PathBuf::from(artifact.source).with_extension("rs"))
        .collect::<BTreeSet<_>>();
    let actual_outputs = read_directory_files(generated.path(), "rs")?;
    if actual_outputs != expected_outputs {
        return Err(parser_error(format!(
            "LALRPOP did not generate the complete declared parser set; expected [{}], found [{}]",
            display_paths(&expected_outputs),
            display_paths(&actual_outputs)
        )));
    }

    let mut parsers = BTreeMap::new();
    for artifact in GRAMMAR_ARTIFACTS {
        let generated_path = generated
            .path()
            .join(PathBuf::from(artifact.source).with_extension("rs"));
        let bytes = fs::read(&generated_path).map_err(|source| XtaskError::ReadFile {
            path: generated_path.display().to_string(),
            source,
        })?;
        let bytes = canonicalize_generated_parser_bytes(bytes);
        if bytes.is_empty() || !bytes.starts_with(b"// auto-generated: \"lalrpop ") {
            return Err(parser_error(format!(
                "generated parser has no LALRPOP provenance header: {}",
                generated_path.display()
            )));
        }
        if parsers
            .insert(artifact.generated.to_string(), bytes)
            .is_some()
        {
            return Err(parser_error(format!(
                "duplicate generated parser destination: {}",
                artifact.generated
            )));
        }
    }
    Ok(parsers)
}

fn expected_generated_names() -> BTreeSet<String> {
    GRAMMAR_ARTIFACTS
        .iter()
        .map(|artifact| artifact.generated.to_string())
        .collect()
}

fn validate_parser_set(parsers: &GeneratedParserSet) -> Result<(), XtaskError> {
    let actual = parsers.keys().cloned().collect::<BTreeSet<_>>();
    let expected = expected_generated_names();
    if actual != expected {
        return Err(parser_error(format!(
            "generated parser set is incomplete; expected [{}], found [{}]",
            expected.into_iter().collect::<Vec<_>>().join(", "),
            actual.into_iter().collect::<Vec<_>>().join(", ")
        )));
    }
    if let Some((name, _)) = parsers.iter().find(|(_, bytes)| bytes.is_empty()) {
        return Err(parser_error(format!("generated parser is empty: {name}")));
    }
    Ok(())
}

fn checked_in_drift(root: &Path, expected: &GeneratedParserSet) -> Result<Vec<String>, XtaskError> {
    validate_parser_set(expected)?;
    let directory = root.join(CHECKED_IN_DIR);
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Ok(vec![format!(
                "{} is not a regular directory",
                directory.display()
            )]);
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(vec![format!("{} is missing", directory.display())]);
        }
        Err(source) => {
            return Err(XtaskError::ReadFile {
                path: directory.display().to_string(),
                source,
            });
        }
    };

    let mut actual_names = BTreeSet::new();
    let mut drift = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|source| XtaskError::ReadFile {
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
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            drift.push(format!("non-UTF-8 entry at {}", path.display()));
            continue;
        };
        actual_names.insert(name.clone());
        if !file_type.is_file() || file_type.is_symlink() {
            drift.push(format!("unexpected non-file entry at {}", path.display()));
            continue;
        }
        match expected.get(&name) {
            Some(expected_bytes) => {
                let actual = fs::read(&path).map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })?;
                if actual != *expected_bytes {
                    drift.push(format!("byte drift in {}", path.display()));
                }
            }
            None => drift.push(format!("unexpected generated file {}", path.display())),
        }
    }

    for missing in expected_generated_names().difference(&actual_names) {
        drift.push(format!(
            "missing generated file {}",
            directory.join(missing).display()
        ));
    }
    drift.sort();
    drift.dedup();
    Ok(drift)
}

fn write_staged_parser_set(
    directory: &Path,
    parsers: &GeneratedParserSet,
) -> Result<(), XtaskError> {
    validate_parser_set(parsers)?;
    for (name, bytes) in parsers {
        let path = directory.join(name);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|source| XtaskError::WriteFile {
                path: path.display().to_string(),
                source,
            })?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|source| XtaskError::WriteFile {
                path: path.display().to_string(),
                source,
            })?;
    }
    Ok(())
}

fn install_parser_set(root: &Path, parsers: &GeneratedParserSet) -> Result<(), XtaskError> {
    install_parser_set_with_rename(root, parsers, |from, to| fs::rename(from, to))
}

fn install_parser_set_with_rename<R>(
    root: &Path,
    parsers: &GeneratedParserSet,
    mut rename: R,
) -> Result<(), XtaskError>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
{
    validate_parser_set(parsers)?;
    let destination = root.join(CHECKED_IN_DIR);
    let parent = destination.parent().ok_or_else(|| {
        parser_error(format!(
            "checked-in parser directory has no parent: {}",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;

    let staging = tempfile::Builder::new()
        .prefix(".lalrpop-stage-")
        .tempdir_in(parent)
        .map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    write_staged_parser_set(staging.path(), parsers)?;

    let staged_root = staging.path().to_path_buf();
    let staged_files = fs::read_dir(&staged_root)
        .map_err(|source| XtaskError::ReadFile {
            path: staged_root.display().to_string(),
            source,
        })?
        .count();
    if staged_files != GRAMMAR_ARTIFACTS.len() {
        return Err(parser_error(format!(
            "staged parser directory contains {staged_files} entries, expected {}",
            GRAMMAR_ARTIFACTS.len()
        )));
    }

    let had_destination = match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => true,
        Ok(_) => {
            return Err(parser_error(format!(
                "refusing to replace non-directory parser destination: {}",
                destination.display()
            )));
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => false,
        Err(source) => {
            return Err(XtaskError::ReadFile {
                path: destination.display().to_string(),
                source,
            });
        }
    };

    let rollback = tempfile::Builder::new()
        .prefix(".lalrpop-rollback-")
        .tempdir_in(parent)
        .map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    let previous = rollback.path().join("previous");
    if had_destination {
        rename(&destination, &previous).map_err(|source| XtaskError::WriteFile {
            path: destination.display().to_string(),
            source,
        })?;
    }

    if let Err(source) = rename(&staged_root, &destination) {
        let rollback_error = if had_destination {
            rename(&previous, &destination).err()
        } else {
            None
        };
        return Err(parser_error(match rollback_error {
            Some(rollback_error) => format!(
                "failed to install generated parsers: {source}; failed to restore previous parsers: {rollback_error}; rollback retained at {}",
                rollback.keep().display()
            ),
            None if had_destination => {
                format!("failed to install generated parsers: {source}; previous parsers restored")
            }
            None => format!(
                "failed to install generated parsers: {source}; no previous parser set existed"
            ),
        }));
    }

    rollback.close().map_err(|source| XtaskError::WriteFile {
        path: parent.display().to_string(),
        source,
    })?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    Ok(())
}

pub(crate) fn gen_lalrpop_parsers(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    let parsers = generate_parser_set(&root)?;
    install_parser_set(&root, &parsers)
}

pub(crate) fn verify_lalrpop_parsers_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let expected = generate_parser_set(&root)?;
    let drift = checked_in_drift(&root, &expected)?;
    if drift.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "checked-in LALRPOP parsers drifted: {}; regenerate with `{REGENERATE_COMMAND}`",
            drift.join("; ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_parser_set(marker: &[u8]) -> GeneratedParserSet {
        GRAMMAR_ARTIFACTS
            .iter()
            .map(|artifact| (artifact.generated.to_string(), marker.to_vec()))
            .collect()
    }

    #[test]
    fn committed_checked_in_parsers_are_byte_fresh() {
        assert_eq!(verify_lalrpop_parsers_artifacts().unwrap(), None);
    }

    #[test]
    fn incomplete_parser_set_is_rejected_before_install() {
        let mut parsers = fixture_parser_set(b"parser");
        parsers.remove("state_grammar.rs");
        let error = validate_parser_set(&parsers).unwrap_err().to_string();
        assert!(error.contains("generated parser set is incomplete"));
    }

    #[test]
    fn generated_parser_normalization_removes_only_line_end_whitespace() {
        assert_eq!(
            canonicalize_generated_parser_bytes(b"where \n\t\r\nkeep  spaces\nlast\t".to_vec()),
            b"where\n\r\nkeep  spaces\nlast".to_vec()
        );
    }

    #[test]
    fn grammar_source_normalization_is_platform_independent() {
        assert_eq!(
            normalize_grammar_source_bytes(b"unix\nwindows\r\nlegacy\rend"),
            b"unix\nwindows\nlegacy\nend"
        );
    }

    #[test]
    fn byte_drift_and_extra_files_are_reported() {
        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join(CHECKED_IN_DIR);
        fs::create_dir_all(&destination).unwrap();
        let expected = fixture_parser_set(b"expected");
        for (name, bytes) in &expected {
            fs::write(destination.join(name), bytes).unwrap();
        }
        fs::write(destination.join("class_grammar.rs"), b"drifted").unwrap();
        fs::write(destination.join("unexpected.rs"), b"extra").unwrap();

        let drift = checked_in_drift(temporary.path(), &expected).unwrap();
        assert!(drift.iter().any(|item| item.contains("byte drift")));
        assert!(
            drift
                .iter()
                .any(|item| item.contains("unexpected generated file"))
        );
    }

    #[test]
    fn install_replaces_the_complete_directory_without_stale_files() {
        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join(CHECKED_IN_DIR);
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("stale.rs"), b"stale").unwrap();
        let expected = fixture_parser_set(b"complete");

        install_parser_set(temporary.path(), &expected).unwrap();

        assert_eq!(
            checked_in_drift(temporary.path(), &expected).unwrap(),
            Vec::<String>::new()
        );
        assert!(!destination.join("stale.rs").exists());
    }

    #[test]
    fn failed_directory_install_restores_the_previous_parser_set() {
        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join(CHECKED_IN_DIR);
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("previous.rs"), b"previous").unwrap();
        let expected = fixture_parser_set(b"complete");
        let mut rename_count = 0;

        let error = install_parser_set_with_rename(temporary.path(), &expected, |from, to| {
            rename_count += 1;
            if rename_count == 2 {
                return Err(io::Error::other("simulated install failure"));
            }
            fs::rename(from, to)
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("previous parsers restored"));
        assert_eq!(
            fs::read(destination.join("previous.rs")).unwrap(),
            b"previous"
        );
        assert!(!destination.join("class_grammar.rs").exists());
    }

    #[test]
    fn failed_initial_directory_install_reports_that_no_prior_set_existed() {
        let temporary = tempfile::tempdir().unwrap();
        let expected = fixture_parser_set(b"complete");

        let error = install_parser_set_with_rename(temporary.path(), &expected, |_, _| {
            Err(io::Error::other("simulated initial install failure"))
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("no previous parser set existed"));
        assert!(!temporary.path().join(CHECKED_IN_DIR).exists());
    }

    #[test]
    fn core_manifest_has_no_build_time_parser_generator() {
        let manifest_path = crate::cmd::workspace_root().join("crates/merman-core/Cargo.toml");
        let manifest: toml::Table =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        assert!(manifest.get("build-dependencies").is_none());
        assert!(!manifest_path.parent().unwrap().join("build.rs").exists());
        assert!(
            manifest
                .get("package")
                .and_then(toml::Value::as_table)
                .and_then(|package| package.get("build"))
                .is_none()
        );
    }
}
