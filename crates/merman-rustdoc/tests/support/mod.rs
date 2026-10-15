use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn proc_macro_artifact() -> (Vec<PathBuf>, PathBuf) {
    static ARTIFACT: OnceLock<(Vec<PathBuf>, PathBuf)> = OnceLock::new();
    ARTIFACT.get_or_init(build_proc_macro).clone()
}

pub fn dependency_args(directories: &[PathBuf]) -> Vec<OsString> {
    directories
        .iter()
        .flat_map(|directory| {
            let mut search_path = OsString::from("dependency=");
            search_path.push(directory);
            [OsString::from("-L"), search_path]
        })
        .collect()
}

fn build_proc_macro() -> (Vec<PathBuf>, PathBuf) {
    let features = [
        ("all-diagrams", cfg!(feature = "all-diagrams")),
        ("diagram-agentflow", cfg!(feature = "diagram-agentflow")),
        ("diagram-usecase", cfg!(feature = "diagram-usecase")),
        (
            "diagram-architecture",
            cfg!(feature = "diagram-architecture"),
        ),
        ("diagram-block", cfg!(feature = "diagram-block")),
        ("diagram-c4", cfg!(feature = "diagram-c4")),
        ("diagram-class", cfg!(feature = "diagram-class")),
        ("diagram-cynefin", cfg!(feature = "diagram-cynefin")),
        ("diagram-er", cfg!(feature = "diagram-er")),
        (
            "diagram-event-modeling",
            cfg!(feature = "diagram-event-modeling"),
        ),
        ("diagram-flowchart", cfg!(feature = "diagram-flowchart")),
        ("diagram-gantt", cfg!(feature = "diagram-gantt")),
        ("diagram-git-graph", cfg!(feature = "diagram-git-graph")),
        ("diagram-info", cfg!(feature = "diagram-info")),
        ("diagram-ishikawa", cfg!(feature = "diagram-ishikawa")),
        ("diagram-journey", cfg!(feature = "diagram-journey")),
        ("diagram-kanban", cfg!(feature = "diagram-kanban")),
        ("diagram-mindmap", cfg!(feature = "diagram-mindmap")),
        ("diagram-packet", cfg!(feature = "diagram-packet")),
        ("diagram-pie", cfg!(feature = "diagram-pie")),
        (
            "diagram-quadrant-chart",
            cfg!(feature = "diagram-quadrant-chart"),
        ),
        ("diagram-radar", cfg!(feature = "diagram-radar")),
        ("diagram-railroad", cfg!(feature = "diagram-railroad")),
        ("diagram-requirement", cfg!(feature = "diagram-requirement")),
        ("diagram-sankey", cfg!(feature = "diagram-sankey")),
        ("diagram-sequence", cfg!(feature = "diagram-sequence")),
        ("diagram-state", cfg!(feature = "diagram-state")),
        ("diagram-swimlane", cfg!(feature = "diagram-swimlane")),
        ("diagram-timeline", cfg!(feature = "diagram-timeline")),
        ("diagram-tree-view", cfg!(feature = "diagram-tree-view")),
        ("diagram-treemap", cfg!(feature = "diagram-treemap")),
        ("diagram-venn", cfg!(feature = "diagram-venn")),
        ("diagram-wardley", cfg!(feature = "diagram-wardley")),
        ("diagram-xychart", cfg!(feature = "diagram-xychart")),
        ("diagram-zenuml", cfg!(feature = "diagram-zenuml")),
        ("svg", cfg!(feature = "svg")),
        ("layout-cytoscape", cfg!(feature = "layout-cytoscape")),
        ("layout-elk", cfg!(feature = "layout-elk")),
        ("math", cfg!(feature = "math")),
    ]
    .into_iter()
    .filter_map(|(name, enabled)| enabled.then_some(name))
    .collect::<Vec<_>>()
    .join(",");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let metadata = Command::new(&cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "metadata",
            "--no-deps",
            "--format-version=1",
            "--locked",
            "--offline",
        ])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .expect("run Cargo to locate its configured target directory");
    assert!(
        metadata.status.success(),
        "Cargo metadata failed: {}\nstdout:\n{}\nstderr:\n{}",
        metadata.status,
        String::from_utf8_lossy(&metadata.stdout),
        String::from_utf8_lossy(&metadata.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&metadata.stdout).expect("Cargo metadata JSON");
    let target_dir = metadata["target_directory"]
        .as_str()
        .expect("Cargo metadata must report its configured target directory");
    let output = Command::new(&cargo)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "build",
            "--locked",
            "-p",
            "merman-rustdoc",
            "--lib",
            "--no-default-features",
            "--features",
            &features,
            "--message-format=json",
        ])
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .output()
        .expect("run Cargo to locate the matching proc-macro artifact");
    assert!(
        output.status.success(),
        "building rustdoc test proc macro failed: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut artifact = None;
    let mut dependencies = BTreeSet::new();
    for line in String::from_utf8(output.stdout)
        .expect("Cargo JSON must be UTF-8")
        .lines()
    {
        let message: serde_json::Value = serde_json::from_str(line).expect("Cargo JSON message");
        if message["reason"] != "compiler-artifact" {
            continue;
        }
        let is_proc_macro = message["target"]["name"] == "merman_rustdoc"
            && message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "proc-macro"));
        for filename in message["filenames"]
            .as_array()
            .expect("Cargo compiler artifact filenames")
        {
            let path = PathBuf::from(filename.as_str().expect("Cargo artifact filename"));
            dependencies.insert(
                path.parent()
                    .expect("Cargo artifact must have a parent directory")
                    .to_path_buf(),
            );
            if is_proc_macro
                && path.extension() == Some(OsStr::new(std::env::consts::DLL_EXTENSION))
            {
                artifact = Some(path);
            }
        }
    }
    (
        dependencies.into_iter().collect(),
        artifact.expect("Cargo must report the matching merman-rustdoc proc-macro library"),
    )
}
