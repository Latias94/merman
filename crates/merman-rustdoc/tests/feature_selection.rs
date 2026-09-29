use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

mod support;
use support::proc_macro_artifact;

#[test]
fn proc_macro_accepts_only_selected_diagram_families() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp = TempDir(std::env::temp_dir().join(format!(
        "merman-rustdoc-feature-selection-{}-{now}",
        std::process::id()
    )));
    fs::create_dir(&temp.0).unwrap();
    let (dependencies, artifact) = proc_macro_artifact();

    for (family, selected, source, diagram_type) in [
        (
            "flowchart",
            cfg!(feature = "diagram-flowchart"),
            "flowchart TD\nA[Selected family] --> B[Rendered]\n",
            "flowchart-v2",
        ),
        (
            "gantt",
            cfg!(feature = "diagram-gantt"),
            "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Build\nFeature check :task, 2026-01-01, 1d\n",
            "gantt",
        ),
    ] {
        let source_path = temp.0.join(format!("{family}.rs"));
        let documentation = format!("```mermaid\n{source}```");
        fs::write(
            &source_path,
            format!(
                "#[merman_rustdoc::merman(theme = \"default\")]\n#[doc = {documentation:?}]\npub fn selected_diagram() {{}}\n"
            ),
        )
        .unwrap();
        let output_dir = temp.0.join(format!("{family}-doc"));
        let output = Command::new(std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into()))
            .arg("--edition=2024")
            .args(["--crate-name", family])
            .arg("--extern")
            .arg(format!("merman_rustdoc={}", artifact.display()))
            .arg("-L")
            .arg(format!("dependency={}", dependencies.display()))
            .arg("-o")
            .arg(&output_dir)
            .arg(&source_path)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if selected {
            assert!(
                output.status.success(),
                "selected {family} failed: {stderr}"
            );
            let html = fs::read_to_string(output_dir.join(family).join("fn.selected_diagram.html"))
                .unwrap();
            assert!(
                html.contains(r#"class="merman-rustdoc-diagram""#),
                "{family}"
            );
            assert!(html.contains("<svg"), "selected {family} must render SVG");
        } else {
            assert!(!output.status.success(), "unselected {family} was accepted");
            assert!(
                stderr.contains(&format!("Unsupported diagram type: {diagram_type}")),
                "unselected {family} must report its missing parser: {stderr}"
            );
        }
    }
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
