use std::path::PathBuf;

pub(crate) struct CompareDiagramPaths {
    pub fixtures_dir: PathBuf,
    pub upstream_dir: PathBuf,
    pub out_path: PathBuf,
    pub out_svg_dir: PathBuf,
}

pub(crate) fn compare_diagram_paths(
    diagram: &str,
    out_path: Option<PathBuf>,
) -> CompareDiagramPaths {
    let workspace_root = crate::cmd::workspace_root();
    let fixtures_dir = crate::cmd::fixtures_root().join(diagram);
    let upstream_dir = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join(diagram);
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
