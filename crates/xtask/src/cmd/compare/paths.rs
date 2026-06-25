use std::path::PathBuf;

pub(crate) struct CompareDiagramPaths {
    pub fixtures_dir: PathBuf,
    pub upstream_dir: PathBuf,
    pub out_path: PathBuf,
    pub out_svg_dir: PathBuf,
}

pub(crate) fn compare_diagram_paths_with_roots(
    diagram: &str,
    out_path: Option<PathBuf>,
    fixtures_root: Option<PathBuf>,
    upstream_root: Option<PathBuf>,
) -> CompareDiagramPaths {
    let fixtures_root = resolve_compare_root(fixtures_root, crate::cmd::fixtures_root());
    let upstream_root = resolve_compare_root(
        upstream_root,
        crate::cmd::fixtures_root().join("upstream-svgs"),
    );
    let workspace_root = crate::cmd::workspace_root();
    let fixtures_dir = fixtures_root.join(diagram);
    let upstream_dir = upstream_root.join(diagram);
    let out_path = out_path.unwrap_or_else(|| {
        crate::cmd::target_root()
            .join("compare")
            .join(format!("{diagram}_report.md"))
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join(diagram);

    CompareDiagramPaths {
        fixtures_dir,
        upstream_dir,
        out_path,
        out_svg_dir,
    }
}

fn resolve_compare_root(raw: Option<PathBuf>, default: PathBuf) -> PathBuf {
    let Some(raw) = raw else {
        return default;
    };
    if raw.is_absolute() {
        return raw;
    }
    if raw.as_os_str().is_empty() {
        return default;
    }
    crate::cmd::workspace_root().join(raw)
}
