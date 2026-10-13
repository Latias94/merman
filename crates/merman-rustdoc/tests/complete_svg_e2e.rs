use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

mod support;
use support::proc_macro_artifact;

#[test]
fn complete_svg_profile_renders_each_optional_engine() {
    let temp = TempDir::new().expect("create unique test directory");
    let source = temp.path().join("lib.rs");
    let out_dir = temp.path().join("doc");
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
        .args(support::dependency_args(&deps_dir))
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
}

fn assert_doc_contains_svg(out_dir: &Path, page: &str) {
    let relative = format!("merman_rustdoc_complete_svg_e2e/{page}");
    let html = fs::read_to_string(out_dir.join(&relative))
        .unwrap_or_else(|error| panic!("failed to read rustdoc HTML `{relative}`: {error}"));
    assert!(html.contains(r#"class="merman-rustdoc-diagram""#));
    assert!(html.contains("<svg"));
}
