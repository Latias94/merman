use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn complete_svg_profile_renders_each_optional_engine() {
    let temp = unique_temp_dir();
    let source = temp.join("lib.rs");
    let out_dir = temp.join("doc");
    fs::create_dir_all(&temp).unwrap();
    fs::write(
        &source,
        r####"
#[merman_rustdoc::merman]
/// ```mermaid
/// architecture-beta
///   group api(cloud)[API]
///   service server(server)[Server] in api
/// ```
pub fn architecture_diagram() {}

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart-elk TD
///   A --> B
/// ```
pub fn elk_diagram() {}

#[merman_rustdoc::merman]
/// ```mermaid
/// flowchart TD
///   A["$$x^2$$"] --> B
/// ```
pub fn math_diagram() {}
"####,
    )
    .unwrap();

    let (deps_dir, macro_artifact) = proc_macro_artifact();
    let rustdoc = std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into());
    let output = Command::new(rustdoc)
        .arg("--edition=2024")
        .arg("--crate-name")
        .arg("merman_rustdoc_complete_svg_e2e")
        .arg("--extern")
        .arg(format!("merman_rustdoc={}", macro_artifact.display()))
        .arg("-L")
        .arg(format!("dependency={}", deps_dir.display()))
        .arg("-o")
        .arg(&out_dir)
        .arg(&source)
        .output()
        .unwrap();

    if !output.status.success() {
        panic!(
            "rustdoc complete SVG e2e failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for page in [
        "fn.architecture_diagram.html",
        "fn.elk_diagram.html",
        "fn.math_diagram.html",
    ] {
        assert_doc_contains_svg(&out_dir, page);
    }

    let _ = fs::remove_dir_all(temp);
}

fn assert_doc_contains_svg(out_dir: &Path, page: &str) {
    let relative = format!("merman_rustdoc_complete_svg_e2e/{page}");
    let html = fs::read_to_string(out_dir.join(&relative))
        .unwrap_or_else(|error| panic!("failed to read rustdoc HTML `{relative}`: {error}"));
    assert!(html.contains(r#"class="merman-rustdoc-diagram""#));
    assert!(html.contains("<svg"));
}

fn proc_macro_artifact() -> (PathBuf, PathBuf) {
    let deps_dir = std::env::current_exe()
        .expect("current rustdoc e2e test executable")
        .parent()
        .expect("rustdoc e2e test executable must be inside a deps directory")
        .to_path_buf();
    let extension = std::env::consts::DLL_EXTENSION;
    let artifact = fs::read_dir(&deps_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_proc_macro_artifact(path, extension))
        .max_by_key(|path| {
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH)
        })
        .unwrap_or_else(|| {
            panic!(
                "failed to find compiled merman_rustdoc proc-macro artifact in {}",
                deps_dir.display()
            )
        });
    (deps_dir, artifact)
}

fn is_proc_macro_artifact(path: &Path, extension: &str) -> bool {
    let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    path.extension().and_then(OsStr::to_str) == Some(extension)
        && file_name.contains("merman_rustdoc")
}

fn unique_temp_dir() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "merman-rustdoc-complete-svg-e2e-{}-{now}",
        std::process::id()
    ))
}
