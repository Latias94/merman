use std::fs;
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    crate::cmd::fixtures_root()
}

fn upstream_svg_path(diagram_dir: &str, stem: &str) -> PathBuf {
    fixtures_root()
        .join("upstream-svgs")
        .join(diagram_dir)
        .join(format!("{stem}.svg"))
}

fn deferred_fixture_dir(diagram_dir: &str) -> PathBuf {
    fixtures_root().join("_deferred").join(diagram_dir)
}

fn deferred_upstream_svg_dir(diagram_dir: &str) -> PathBuf {
    fixtures_root()
        .join("_deferred")
        .join("upstream-svgs")
        .join(diagram_dir)
}

fn golden_json_path(diagram_dir: &str, stem: &str) -> PathBuf {
    fixtures_root()
        .join(diagram_dir)
        .join(format!("{stem}.golden.json"))
}

fn layout_golden_json_path(diagram_dir: &str, stem: &str) -> PathBuf {
    fixtures_root()
        .join(diagram_dir)
        .join(format!("{stem}.layout.golden.json"))
}

fn move_or_copy_then_remove(src: &Path, dst: &Path) {
    if dst.exists() {
        let _ = fs::remove_file(src);
        return;
    }

    let _ = fs::rename(src, dst)
        .or_else(|_| fs::copy(src, dst).map(|_| ()))
        .and_then(|_| fs::remove_file(src));
}

pub(crate) fn cleanup_fixture_files(diagram_dir: &str, stem: &str, path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(upstream_svg_path(diagram_dir, stem));
    let _ = fs::remove_file(golden_json_path(diagram_dir, stem));
    let _ = fs::remove_file(layout_golden_json_path(diagram_dir, stem));
}

pub(crate) fn defer_fixture_files(
    diagram_dir: &str,
    stem: &str,
    path: &Path,
    keep_upstream_svg: bool,
) -> PathBuf {
    let deferred_fixture_dir = deferred_fixture_dir(diagram_dir);
    let _ = fs::create_dir_all(&deferred_fixture_dir);

    let deferred_fixture_path = deferred_fixture_dir.join(format!("{stem}.mmd"));
    move_or_copy_then_remove(path, &deferred_fixture_path);

    if keep_upstream_svg {
        let upstream_svg_path = upstream_svg_path(diagram_dir, stem);
        if upstream_svg_path.exists() {
            let deferred_svg_dir = deferred_upstream_svg_dir(diagram_dir);
            let _ = fs::create_dir_all(&deferred_svg_dir);

            let deferred_svg_path = deferred_svg_dir.join(format!("{stem}.svg"));
            move_or_copy_then_remove(&upstream_svg_path, &deferred_svg_path);
        }
    } else {
        let _ = fs::remove_file(upstream_svg_path(diagram_dir, stem));
    }

    let _ = fs::remove_file(golden_json_path(diagram_dir, stem));
    let _ = fs::remove_file(layout_golden_json_path(diagram_dir, stem));

    deferred_fixture_path
}
