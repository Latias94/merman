use std::fs;
use std::path::{Path, PathBuf};

fn file_name_contains(path: &Path, needle: &str) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains(needle))
}

pub(crate) fn is_parser_only_fixture(path: &Path) -> bool {
    file_name_contains(path, "_parser_only_") || file_name_contains(path, "_parser_only_spec")
}

pub(crate) fn list_mmd_fixtures_in_dir(
    dir: &Path,
    filter: Option<&str>,
    skip_parser_only: bool,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }
        if skip_parser_only && is_parser_only_fixture(&path) {
            continue;
        }
        if let Some(filter) = filter {
            if !file_name_contains(&path, filter) {
                continue;
            }
        }
        out.push(path);
    }

    out.sort();
    out
}
