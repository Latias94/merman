use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn proc_macro_artifact() -> (PathBuf, PathBuf) {
    static ARTIFACT: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
    ARTIFACT.get_or_init(build_proc_macro).clone()
}

fn build_proc_macro() -> (PathBuf, PathBuf) {
    let features = [
        ("svg", cfg!(feature = "svg")),
        ("layout-cytoscape", cfg!(feature = "layout-cytoscape")),
        ("layout-elk", cfg!(feature = "layout-elk")),
        ("math", cfg!(feature = "math")),
    ]
    .into_iter()
    .filter_map(|(name, enabled)| enabled.then_some(name))
    .collect::<Vec<_>>()
    .join(",");
    let executable = std::env::current_exe().expect("current rustdoc integration test executable");
    let target_dir = executable
        .ancestors()
        .nth(3)
        .expect("test executable must be inside a Cargo profile's deps directory");
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let output = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
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
    let artifact = String::from_utf8(output.stdout)
        .expect("Cargo JSON must be UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("Cargo JSON message"))
        .filter(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "merman_rustdoc"
                && message["target"]["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "proc-macro"))
        })
        .find_map(|message| {
            message["filenames"]
                .as_array()?
                .iter()
                .find_map(|filename| {
                    let path = PathBuf::from(filename.as_str()?);
                    (path.extension() == Some(OsStr::new(std::env::consts::DLL_EXTENSION)))
                        .then_some(path)
                })
        })
        .expect("Cargo must report the matching merman-rustdoc proc-macro library");
    let artifact_dir = artifact
        .parent()
        .expect("proc-macro artifact must have a parent directory");
    let dependencies = if artifact_dir.file_name() == Some(OsStr::new("deps")) {
        artifact_dir.to_path_buf()
    } else {
        artifact_dir.join("deps")
    };
    (dependencies, artifact)
}
