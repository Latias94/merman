use std::fs;
use std::process::Command;
use tempfile::TempDir;

mod support;
use support::proc_macro_artifact;

#[test]
fn proc_macro_accepts_only_selected_diagram_families() {
    let temp = TempDir::new().expect("create unique test directory");
    let (dependencies, artifact) = proc_macro_artifact();

    for (family, selected, source, diagram_type) in [
        (
            "agentflow",
            cfg!(feature = "diagram-agentflow"),
            "agentflow-beta\nA[Selected family] --> B[Rendered]\n",
            "agentflow",
        ),
        (
            "usecase",
            cfg!(feature = "diagram-usecase"),
            "usecase-beta\nactor User\nUser --> Login(Sign in)\n",
            "usecase",
        ),
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
        let source_path = temp.path().join(format!("{family}.rs"));
        let documentation = format!("```mermaid\n{source}```");
        fs::write(
            &source_path,
            format!(
                "#[merman_rustdoc::merman(theme = \"default\")]\n#[doc = {documentation:?}]\npub fn selected_diagram() {{}}\n"
            ),
        )
        .unwrap();
        let output_dir = temp.path().join(format!("{family}-doc"));
        let output = Command::new(std::env::var_os("RUSTDOC").unwrap_or_else(|| "rustdoc".into()))
            .arg("--edition=2024")
            .args(["--crate-name", family])
            .arg("--extern")
            .arg(format!("merman_rustdoc={}", artifact.display()))
            .args(support::dependency_args(&dependencies))
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
