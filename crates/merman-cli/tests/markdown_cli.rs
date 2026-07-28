#![cfg(feature = "markdown")]

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const TWO_CHARTS: &str = concat!(
    "# Diagrams\n\n",
    "```mermaid\n",
    "flowchart LR\n",
    "A[First]-->B[One]\n",
    "```\n\n",
    "```mermaid\n",
    "sequenceDiagram\n",
    "Alice->>Bob: Second\n",
    "```\n",
);

fn cli() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("merman-cli").to_path_buf()
}

fn run_in(directory: &Path, args: &[&str]) -> Output {
    Command::new(cli())
        .current_dir(directory)
        .args(args)
        .output()
        .expect("run merman-cli")
}

fn run_with_stdin(directory: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(cli())
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn merman-cli");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin.as_bytes())
        .expect("write child stdin");
    child.wait_with_output().expect("wait for merman-cli")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn exit_code(output: &Output) -> i32 {
    output.status.code().unwrap_or(-1)
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "exit={:?}\nstderr:\n{}",
        output.status.code(),
        stderr(output)
    );
}

fn manifest(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&fs::read(path).expect("read generation manifest"))
        .expect("parse generation manifest")
}

fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(directory)
            .expect("read snapshot directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read snapshot entries");
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let file_type = entry.file_type().expect("snapshot file type");
            if file_type.is_dir() {
                visit(root, &entry.path(), out);
            } else if file_type.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("snapshot entry under root")
                    .to_path_buf();
                out.insert(
                    relative,
                    fs::read(entry.path()).expect("read snapshot file"),
                );
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

#[test]
fn native_zero_chart_generation_publishes_document_and_cleans_only_owned_stale_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("input.md"), TWO_CHARTS).expect("write Markdown input");

    let first = run_in(
        temp.path(),
        &["batch", "input.md", "--output-dir", "generated", "--quiet"],
    );
    assert_success(&first);

    let output_root = temp.path().join("generated");
    assert!(output_root.join("input-1.svg").is_file());
    assert!(output_root.join("input-2.svg").is_file());
    fs::write(output_root.join("unowned.keep"), b"not owned by Merman")
        .expect("write unowned file");

    let empty = "# No diagrams\n\nPlain text.\n";
    fs::write(temp.path().join("input.md"), empty).expect("replace Markdown input");
    let second = run_in(
        temp.path(),
        &["batch", "input.md", "--output-dir", "generated", "--quiet"],
    );
    assert_success(&second);

    assert_eq!(
        fs::read_to_string(output_root.join("input.md")).expect("read rewritten document"),
        empty
    );
    assert!(!output_root.join("input-1.svg").exists());
    assert!(!output_root.join("input-2.svg").exists());
    assert_eq!(
        fs::read(output_root.join("unowned.keep")).expect("read unowned file"),
        b"not owned by Merman"
    );
    assert!(output_root.join(".merman.lock").is_file());
    assert!(!output_root.join(".merman.transaction").exists());

    let manifest = manifest(&output_root.join(".merman-manifest.json"));
    assert_eq!(manifest["owner"]["dialect"], "native_batch_v1");
    assert_eq!(manifest["artifacts"].as_array().map(Vec::len), Some(0));
    assert!(!manifest["document"].is_null());
}

#[test]
fn native_zero_chart_generation_skips_renderer_configuration() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("input.md"), "# No diagrams\n").expect("write Markdown input");
    fs::write(temp.path().join("invalid.json"), b"not valid JSON")
        .expect("write invalid configuration");

    let output = run_in(
        temp.path(),
        &[
            "batch",
            "input.md",
            "--output-dir",
            "generated",
            "--config-file",
            "invalid.json",
            "--quiet",
        ],
    );
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(temp.path().join("generated/input.md"))
            .expect("read rewritten document"),
        "# No diagrams\n"
    );
    assert!(
        temp.path()
            .join("generated/.merman-manifest.json")
            .is_file()
    );
}

#[test]
fn strict_document_generations_never_delete_extra_numbered_outputs() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("input.md"), TWO_CHARTS).expect("write Markdown input");

    let first = run_in(
        temp.path(),
        &["mmdc", "-i", "input.md", "-o", "out.md", "--quiet"],
    );
    assert_success(&first);
    let second_artifact = fs::read(temp.path().join("out-2.svg")).expect("read second artefact");

    fs::write(
        temp.path().join("input.md"),
        "```mermaid\nflowchart LR\nA-->B\n```\n",
    )
    .expect("write one-chart input");
    let second = run_in(
        temp.path(),
        &["mmdc", "-i", "input.md", "-o", "out.md", "--quiet"],
    );
    assert_success(&second);
    assert_eq!(
        fs::read(temp.path().join("out-2.svg")).expect("read retained second artefact"),
        second_artifact,
        "strict mmdc shrink must retain extra numbered outputs"
    );

    let empty = "# No diagrams\n";
    fs::write(temp.path().join("input.md"), empty).expect("write zero-chart input");
    let third = run_in(
        temp.path(),
        &["mmdc", "-i", "input.md", "-o", "out.md", "--quiet"],
    );
    assert_success(&third);
    assert_eq!(
        fs::read_to_string(temp.path().join("out.md")).expect("read strict document"),
        empty
    );
    assert!(temp.path().join("out-1.svg").is_file());
    assert_eq!(
        fs::read(temp.path().join("out-2.svg")).expect("read retained second artefact"),
        second_artifact
    );

    let manifest = manifest(&temp.path().join("out.md.merman-manifest.json"));
    assert_eq!(manifest["owner"]["dialect"], "mmdc11_16_0");
    assert_eq!(manifest["artifacts"].as_array().map(Vec::len), Some(0));
}

#[test]
fn strict_image_zero_chart_run_only_locks_and_recovers() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("input.md"), "# Nothing to render\n").expect("write input");
    fs::write(temp.path().join("out.svg"), b"template sentinel").expect("write template");
    fs::write(
        temp.path().join("out.svg.merman-manifest.json"),
        b"manifest sentinel that is deliberately not JSON",
    )
    .expect("write manifest sentinel");
    fs::write(temp.path().join("out-9.svg"), b"stale sentinel").expect("write stale sentinel");

    let output = run_in(temp.path(), &["mmdc", "-i", "input.md", "-o", "out.svg"]);
    assert_success(&output);
    assert!(stderr(&output).contains("No mermaid charts found in Markdown input"));
    assert_eq!(
        fs::read(temp.path().join("out.svg")).expect("read template"),
        b"template sentinel"
    );
    assert_eq!(
        fs::read(temp.path().join("out.svg.merman-manifest.json")).expect("read manifest"),
        b"manifest sentinel that is deliberately not JSON"
    );
    assert_eq!(
        fs::read(temp.path().join("out-9.svg")).expect("read stale artifact"),
        b"stale sentinel"
    );
    assert!(!temp.path().join("out-1.svg").exists());
    assert!(temp.path().join(".merman.lock").is_file());
    assert!(!temp.path().join(".merman.transaction").exists());
}

#[test]
fn strict_split_root_is_rejected_before_any_publication_side_effect() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("docs")).expect("create docs");
    fs::write(temp.path().join("docs/input.md"), TWO_CHARTS).expect("write input");

    let output = run_in(
        temp.path(),
        &[
            "mmdc",
            "-i",
            "docs/input.md",
            "-o",
            "docs/out.md",
            "--artefacts",
            "assets",
        ],
    );

    assert_eq!(exit_code(&output), 2, "stderr:\n{}", stderr(&output));
    assert!(
        stderr(&output).contains("split-root"),
        "unexpected stderr:\n{}",
        stderr(&output)
    );
    assert!(!temp.path().join("assets").exists());
    assert!(!temp.path().join("docs/.merman.lock").exists());
    assert!(!temp.path().join("docs/out.md").exists());
    assert!(
        !temp
            .path()
            .join("docs/out.md.merman-manifest.json")
            .exists()
    );
}

#[test]
fn strict_nested_artefacts_directory_stays_inside_the_document_transaction() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("docs")).expect("create docs");
    fs::write(temp.path().join("docs/input.md"), TWO_CHARTS).expect("write input");

    let output = run_in(
        temp.path(),
        &[
            "mmdc",
            "-i",
            "docs/input.md",
            "-o",
            "docs/out.md",
            "--artefacts",
            "docs/assets",
            "--quiet",
        ],
    );
    assert_success(&output);

    assert!(temp.path().join("docs/assets/out-1.svg").is_file());
    assert!(temp.path().join("docs/assets/out-2.svg").is_file());
    let rewritten =
        fs::read_to_string(temp.path().join("docs/out.md")).expect("read strict rewrite");
    assert!(rewritten.contains("![diagram](./assets/out-1.svg)"));
    assert!(rewritten.contains("![diagram](./assets/out-2.svg)"));
    assert!(temp.path().join("docs/.merman.lock").is_file());
    assert!(!temp.path().join("docs/assets/.merman.lock").exists());
}

#[cfg(unix)]
#[test]
fn strict_nested_artefacts_symlink_keeps_requested_url_and_uses_approved_directory() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("docs")).expect("create docs");
    fs::create_dir(temp.path().join("docs/real-assets")).expect("create real artefacts directory");
    symlink("real-assets", temp.path().join("docs/assets")).expect("create artefacts symlink");
    fs::write(temp.path().join("docs/input.md"), TWO_CHARTS).expect("write input");

    let output = run_in(
        temp.path(),
        &[
            "mmdc",
            "-i",
            "docs/input.md",
            "-o",
            "docs/out.md",
            "--artefacts",
            "docs/assets",
            "--quiet",
        ],
    );
    assert_success(&output);

    assert!(temp.path().join("docs/real-assets/out-1.svg").is_file());
    assert!(temp.path().join("docs/real-assets/out-2.svg").is_file());
    let rewritten =
        fs::read_to_string(temp.path().join("docs/out.md")).expect("read strict rewrite");
    assert!(rewritten.contains("![diagram](./assets/out-1.svg)"));
    assert!(rewritten.contains("![diagram](./assets/out-2.svg)"));
}

#[test]
fn malformed_or_wrong_owner_manifest_fails_closed_before_rendering() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("input.md"), TWO_CHARTS).expect("write input");
    fs::write(
        temp.path().join("out.md.merman-manifest.json"),
        b"not valid JSON",
    )
    .expect("write malformed manifest");

    let malformed = run_in(
        temp.path(),
        &["mmdc", "-i", "input.md", "-o", "out.md", "--quiet"],
    );
    assert_eq!(exit_code(&malformed), 3, "stderr:\n{}", stderr(&malformed));
    assert!(!temp.path().join("out.md").exists());
    assert!(!temp.path().join("out-1.svg").exists());
    assert!(!temp.path().join(".merman.transaction").exists());

    fs::remove_file(temp.path().join("out.md.merman-manifest.json"))
        .expect("remove malformed manifest");
    let baseline = run_in(
        temp.path(),
        &["mmdc", "-i", "input.md", "-o", "out.md", "--quiet"],
    );
    assert_success(&baseline);
    fs::copy(
        temp.path().join("out.md.merman-manifest.json"),
        temp.path().join("other.md.merman-manifest.json"),
    )
    .expect("copy manifest to another owner");

    let wrong_owner = run_in(
        temp.path(),
        &["mmdc", "-i", "input.md", "-o", "other.md", "--quiet"],
    );
    assert_eq!(
        exit_code(&wrong_owner),
        3,
        "stderr:\n{}",
        stderr(&wrong_owner)
    );
    assert!(stderr(&wrong_owner).contains("different Markdown dialect, owner"));
    assert!(!temp.path().join("other.md").exists());
    assert!(!temp.path().join("other-1.svg").exists());
    assert!(!temp.path().join(".merman.transaction").exists());
}

#[test]
fn render_failures_preserve_the_complete_previous_generation() {
    let temp = tempfile::tempdir().expect("tempdir");
    let input = temp.path().join("input.md");
    fs::write(
        &input,
        concat!(
            "```mermaid\nflowchart LR\nA-->B\n```\n",
            "```mermaid\nflowchart LR\nB-->C\n```\n",
            "```mermaid\nflowchart LR\nC-->D\n```\n",
        ),
    )
    .expect("write baseline");
    let baseline = run_in(
        temp.path(),
        &["batch", "input.md", "--output-dir", "generated", "--quiet"],
    );
    assert_success(&baseline);
    let output_root = temp.path().join("generated");
    let expected = snapshot_tree(&output_root);

    let job_counts = [
        None,
        #[cfg(feature = "parallel-markdown")]
        Some("3"),
    ];
    for jobs in job_counts {
        for failed_index in 0..3 {
            let definitions = [
                "flowchart LR\nA-->B\n",
                "flowchart LR\nB-->C\n",
                "flowchart LR\nC-->D\n",
            ];
            let mut source = String::new();
            for (index, definition) in definitions.iter().enumerate() {
                source.push_str("```mermaid\n");
                if index == failed_index {
                    source.push_str("definitely not a Mermaid diagram\n");
                } else {
                    source.push_str(definition);
                }
                source.push_str("```\n");
            }
            fs::write(&input, source).expect("write failing generation");

            let mut args = vec!["batch", "input.md", "--output-dir", "generated"];
            if let Some(jobs) = jobs {
                args.extend(["--jobs", jobs]);
            }
            args.push("--quiet");
            let output = run_in(temp.path(), &args);
            assert_eq!(exit_code(&output), 1, "stderr:\n{}", stderr(&output));
            assert!(
                stderr(&output).contains(&format!("Markdown chart {}", failed_index + 1)),
                "error must be stable by source index:\n{}",
                stderr(&output)
            );
            assert_eq!(
                snapshot_tree(&output_root),
                expected,
                "failed chart {failed_index} with jobs={jobs:?} changed the prior generation"
            );
            assert!(!output_root.join(".merman.transaction").exists());
        }
    }
}

#[cfg(feature = "parallel-markdown")]
#[test]
fn parallel_batch_rewrite_and_manifest_use_source_order() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(
        temp.path().join("ordered.md"),
        concat!(
            "one\n```mermaid\nflowchart LR\nA-->B\n```\n",
            "two\n```mermaid\nflowchart LR\nB-->C\n```\n",
            "three\n```mermaid\nflowchart LR\nC-->D\n```\n",
        ),
    )
    .expect("write Markdown input");

    let output = run_in(
        temp.path(),
        &[
            "batch",
            "ordered.md",
            "--output-dir",
            "ordered.merman",
            "--jobs",
            "3",
            "--quiet",
        ],
    );
    assert_success(&output);

    let rewritten =
        fs::read_to_string(temp.path().join("ordered.merman/ordered.md")).expect("read rewrite");
    let first = rewritten.find("./ordered-1.svg").expect("first URL");
    let second = rewritten.find("./ordered-2.svg").expect("second URL");
    let third = rewritten.find("./ordered-3.svg").expect("third URL");
    assert!(first < second && second < third, "{rewritten}");

    let manifest = manifest(&temp.path().join("ordered.merman/.merman-manifest.json"));
    assert_eq!(manifest["artifacts"].as_array().map(Vec::len), Some(3));
    assert!(
        !temp
            .path()
            .join("ordered.merman/.merman.transaction")
            .exists()
    );
}

#[cfg(all(feature = "parallel-markdown", feature = "pdf"))]
#[test]
fn parallel_pdf_batch_publishes_numbered_pdf_artifacts() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("input.md"), TWO_CHARTS).expect("write Markdown input");

    let output = run_in(
        temp.path(),
        &[
            "batch",
            "input.md",
            "--output-dir",
            "pdf.merman",
            "--format",
            "pdf",
            "--jobs",
            "2",
            "--quiet",
        ],
    );
    assert_success(&output);

    for index in 1..=2 {
        let bytes =
            fs::read(temp.path().join(format!("pdf.merman/input-{index}.pdf"))).expect("read PDF");
        assert!(bytes.starts_with(b"%PDF-"), "artifact {index} is not a PDF");
    }
    let rewritten =
        fs::read_to_string(temp.path().join("pdf.merman/input.md")).expect("read rewrite");
    assert!(rewritten.contains("![diagram](./input-1.pdf)"));
    assert!(rewritten.contains("![diagram](./input-2.pdf)"));
}

#[test]
fn native_batch_stdin_requires_a_file_name_and_output_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = run_with_stdin(
        temp.path(),
        &["batch", "-"],
        "```mermaid\nflowchart LR\nA-->B\n```\n",
    );

    assert_eq!(exit_code(&output), 2, "stderr:\n{}", stderr(&output));
    assert!(stderr(&output).contains("--stdin-file-name"));
    assert!(stderr(&output).contains("--output-dir"));
    assert_eq!(
        fs::read_dir(temp.path()).expect("read tempdir").count(),
        0,
        "invalid stdin invocation must not create output state"
    );
}
