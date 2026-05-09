use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

pub(crate) fn fixtures_root_for_diagram(workspace_root: &Path, diagram: &str) -> PathBuf {
    if diagram == "all" {
        workspace_root.join("fixtures")
    } else {
        workspace_root.join("fixtures").join(diagram)
    }
}

fn file_name_contains(path: &Path, needle: &str) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains(needle))
}

pub(crate) fn is_parser_only_fixture(path: &Path) -> bool {
    file_name_contains(path, "_parser_only_") || file_name_contains(path, "_parser_only_spec")
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MmdFixtureScan<'a> {
    pub filter: Option<&'a str>,
    pub recursive: bool,
    pub skip_private_dirs: bool,
    pub skip_parser_only: bool,
    pub skip_upstream_svgs: bool,
}

fn is_private_fixture_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('_'))
}

fn is_upstream_svg_dir(path: &Path) -> bool {
    path.file_name().is_some_and(|n| n == "upstream-svgs")
}

pub(crate) fn collect_mmd_fixtures(root: &Path, scan: MmdFixtureScan<'_>) -> Vec<PathBuf> {
    let root = root.to_path_buf();
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];

    while let Some(dir) = stack.pop() {
        if dir != root && scan.skip_private_dirs && is_private_fixture_dir(&dir) {
            continue;
        }
        if dir != root && scan.skip_upstream_svgs && is_upstream_svg_dir(&dir) {
            continue;
        }

        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !scan.recursive {
                    continue;
                }
                if scan.skip_private_dirs && is_private_fixture_dir(&path) {
                    continue;
                }
                if scan.skip_upstream_svgs && is_upstream_svg_dir(&path) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "mmd") {
                continue;
            }
            if scan.skip_parser_only && is_parser_only_fixture(&path) {
                continue;
            }
            if let Some(filter) = scan.filter {
                if !file_name_contains(&path, filter) {
                    continue;
                }
            }
            out.push(path);
        }
    }

    out.sort();
    out
}

pub(crate) fn list_mmd_fixtures_in_dir(
    dir: &Path,
    filter: Option<&str>,
    skip_parser_only: bool,
) -> Vec<PathBuf> {
    collect_mmd_fixtures(
        dir,
        MmdFixtureScan {
            filter,
            recursive: false,
            skip_private_dirs: false,
            skip_parser_only,
            skip_upstream_svgs: false,
        },
    )
}
