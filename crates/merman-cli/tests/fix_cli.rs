mod support;

use assert_cmd::cargo::cargo_bin;
use std::fs;
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use support::{exit_code, run_before_stdin_close_in_dir, run_with_stdin, run_with_stdin_bytes};

const DIRECTION_RULE: &str = "merman.authoring.flowchart.explicit_direction";
const INIT_RULE: &str = "merman.authoring.config.prefer_init_directive";
const FRONTMATTER_RULE: &str = "merman.authoring.config.prefer_frontmatter_config";
const FLOWCHART_DIRECTION_FIX: &str =
    "mfix-v1:38351a55ad911a42482b73dc3eef664abd18b0e624a5631a4949f033c5ca15f2";

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout should be UTF-8")
}

#[test]
fn repeated_document_fix_is_applied_once() {
    let source = concat!(
        "%%{ init: {\"theme\":\"dark\"} }%%\n",
        "%%{ init: {\"flowchart\":{\"curve\":\"linear\"}} }%%\n",
        "flowchart TD\n",
        "A-->B\n",
    );
    let output = run_with_stdin(&["fix", "-"], source);

    assert_eq!(exit_code(output.status), 0, "stderr:\n{}", stderr(&output));
    let fixed = stdout(&output);
    assert_eq!(fixed.matches("---\n").count(), 2, "{fixed}");
    assert!(!fixed.contains("%%{"), "{fixed}");
    assert_eq!(fixed.matches("theme: dark").count(), 1, "{fixed}");
}

#[test]
fn successful_stdout_fix_does_not_relint_for_its_exit_code() {
    let source = concat!("flowchart\n", "style Q background:#fff\n", "A-->B\n",);
    let output = run_with_stdin(&["fix", "--rule", DIRECTION_RULE, "-"], source);

    assert_eq!(exit_code(output.status), 0, "stderr:\n{}", stderr(&output));
    assert!(stdout(&output).starts_with("flowchart TB\n"));
}

#[test]
fn check_and_diff_exit_only_on_selected_changes() {
    let changed = "flowchart\nA-->B\n";

    let check = run_with_stdin(&["fix", "--check", "-"], changed);
    assert_eq!(exit_code(check.status), 1, "stderr:\n{}", stderr(&check));
    assert!(check.stdout.is_empty());

    let diff = run_with_stdin(&["fix", "--diff", "-"], changed);
    assert_eq!(exit_code(diff.status), 1, "stderr:\n{}", stderr(&diff));
    let diff = stdout(&diff);
    assert!(diff.starts_with("--- a/stdin\n+++ b/stdin\n"), "{diff}");
    assert!(diff.contains("-flowchart\n+flowchart TB\n"), "{diff}");

    let unchanged = "flowchart TD\nstyle Q background:#fff\nA-->B\n";
    for mode in ["--check", "--diff"] {
        let output = run_with_stdin(&["fix", mode, "-"], unchanged);
        assert_eq!(
            exit_code(output.status),
            0,
            "{mode} stderr:\n{}",
            stderr(&output)
        );
        assert!(output.stdout.is_empty(), "{mode}");
    }
}

#[test]
fn unified_diff_is_stable_for_utf8_without_a_trailing_newline() {
    let output = run_with_stdin(&["fix", "--diff", "-"], "flowchart\nA[\"中文\"]-->B");

    assert_eq!(exit_code(output.status), 1, "stderr:\n{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        concat!(
            "--- a/stdin\n",
            "+++ b/stdin\n",
            "@@ -1,2 +1,2 @@\n",
            "-flowchart\n",
            "+flowchart TB\n",
            " A[\"中文\"]-->B\n",
            "\\ No newline at end of file\n",
        )
    );
}

#[test]
fn rule_selectors_enable_and_filter_fixes_without_corrupting_utf8() {
    let source = concat!(
        "%%{ initialize: {\"theme\":\"dark\"} }%%\n",
        "flowchart\n",
        "A[\"中文\"]-->B\n",
    );
    let common = ["--disable-rule", FRONTMATTER_RULE, "--rule", INIT_RULE, "-"];
    let init_only = run_with_stdin(
        &["fix", common[0], common[1], common[2], common[3], common[4]],
        source,
    );
    assert_eq!(
        exit_code(init_only.status),
        0,
        "stderr:\n{}",
        stderr(&init_only)
    );
    let fixed = stdout(&init_only);
    assert!(fixed.contains("%%{ init:"), "{fixed}");
    assert!(fixed.contains("flowchart\n"), "{fixed}");
    assert!(fixed.contains("中文"), "{fixed}");

    let direction_only = run_with_stdin(
        &[
            "fix",
            "--disable-rule",
            FRONTMATTER_RULE,
            "--rule",
            DIRECTION_RULE,
            "-",
        ],
        source,
    );
    assert_eq!(
        exit_code(direction_only.status),
        0,
        "stderr:\n{}",
        stderr(&direction_only)
    );
    let fixed = stdout(&direction_only);
    assert!(fixed.contains("initialize"), "{fixed}");
    assert!(fixed.contains("flowchart TB\n"), "{fixed}");
    assert!(fixed.contains("中文"), "{fixed}");
}

#[test]
fn invalid_modes_and_selectors_fail_before_waiting_for_stdin() {
    let directory = tempfile::tempdir().expect("tempdir");
    let malformed_fix = "mfix-v1:abcd";
    let cases: &[&[&str]] = &[
        &["fix", "--check", "--diff", "-"],
        &["fix", "--check", "--write", "-"],
        &["fix", "--check", "--output", "fixed.mmd", "-"],
        &["fix", "--diff", "--write", "-"],
        &["fix", "--diff", "--output", "fixed.mmd", "-"],
        &["fix", "--write", "--output", "fixed.mmd", "-"],
        &["fix", "--write", "-"],
        &["fix", "--output", "-", "-"],
        &["fix", "--all", "-"],
        &["fix", "--rule", "unknown.rule", "-"],
        &[
            "fix",
            "--rule",
            "merman.compatibility.config.deprecated_flowchart_html_labels",
            "-",
        ],
        &["fix", "--fix", malformed_fix, "-"],
    ];

    for args in cases {
        let output =
            run_before_stdin_close_in_dir(args, Some(directory.path()), Duration::from_secs(2));
        assert_eq!(
            exit_code(output.status),
            2,
            "{args:?} stderr:\n{}",
            stderr(&output)
        );
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!directory.path().join("fixed.mmd").exists(), "{args:?}");
    }
}

#[test]
fn unknown_well_formed_fix_id_fails_without_publishing() {
    let unknown = format!("mfix-v1:{}", "0".repeat(64));
    let output = run_with_stdin(
        &["fix", "--fix", unknown.as_str(), "-"],
        "flowchart\nA-->B\n",
    );

    assert_eq!(exit_code(output.status), 2, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("unknown fix id"));

    let directory = tempfile::tempdir().expect("tempdir");
    fs::write(directory.path().join("input.mmd"), "flowchart\nA-->B\n").expect("write source");
    let output = Command::new(cargo_bin("merman-cli"))
        .current_dir(directory.path())
        .args([
            "fix",
            "input.mmd",
            "--fix",
            unknown.as_str(),
            "--output",
            "fixed.mmd",
        ])
        .output()
        .expect("run unknown exact fix");
    assert_eq!(exit_code(output.status), 2, "stderr:\n{}", stderr(&output));
    assert!(!directory.path().join("fixed.mmd").exists());
}

#[test]
fn exact_fix_id_is_stable_selective_and_rule_eligible() {
    let source = "flowchart\nA-->B\n";
    let output = run_with_stdin(
        &[
            "fix",
            "--fix",
            FLOWCHART_DIRECTION_FIX,
            "--fix",
            FLOWCHART_DIRECTION_FIX,
            "-",
        ],
        source,
    );
    assert_eq!(exit_code(output.status), 0, "stderr:\n{}", stderr(&output));
    assert_eq!(stdout(&output), "flowchart TB\nA-->B\n");

    let ineligible = run_with_stdin(
        &[
            "fix",
            "--rule",
            INIT_RULE,
            "--fix",
            FLOWCHART_DIRECTION_FIX,
            "-",
        ],
        source,
    );
    assert_eq!(
        exit_code(ineligible.status),
        2,
        "stderr:\n{}",
        stderr(&ineligible)
    );
    assert!(ineligible.stdout.is_empty());
    assert!(stderr(&ineligible).contains("not eligible"));
}

#[test]
fn file_output_and_write_modes_publish_only_the_requested_target() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    let output_path = tmp.path().join("fixed.mmd");
    let original = "flowchart\nA-->B\n";
    fs::write(&input, original).expect("write source");

    let output = Command::new(cargo_bin("merman-cli"))
        .current_dir(tmp.path())
        .args(["fix", "input.mmd", "--output", "fixed.mmd"])
        .output()
        .expect("run fix output");
    assert_eq!(exit_code(output.status), 0, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_to_string(&input).expect("source"), original);
    assert!(
        fs::read_to_string(&output_path)
            .expect("fixed output")
            .starts_with("flowchart TB\n")
    );

    let output = Command::new(cargo_bin("merman-cli"))
        .current_dir(tmp.path())
        .args(["fix", "input.mmd", "--write"])
        .output()
        .expect("run fix write");
    assert_eq!(exit_code(output.status), 0, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(
        fs::read_to_string(&input)
            .expect("written source")
            .starts_with("flowchart TB\n")
    );
}

#[test]
fn no_change_write_preserves_hard_link_identity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input.mmd");
    let sibling = tmp.path().join("sibling.mmd");
    let source = "flowchart TD\nA-->B\n";
    fs::write(&input, source).expect("write source");
    fs::hard_link(&input, &sibling).expect("create hard link");

    let output = Command::new(cargo_bin("merman-cli"))
        .current_dir(tmp.path())
        .args(["fix", "input.mmd", "--write"])
        .output()
        .expect("run no-change fix");

    assert_eq!(exit_code(output.status), 0, "stderr:\n{}", stderr(&output));
    assert!(output.stdout.is_empty());
    assert!(same_file::is_same_file(&input, &sibling).expect("compare hard links"));
}

#[test]
fn markdown_fix_preserves_the_document_envelope_and_utf8() {
    let source = concat!(
        "before\n\n",
        "```mermaid\n",
        "%%{ initialize: {\"theme\":\"dark\"} }%%\n",
        "flowchart TD\n",
        "A[\"中文\"]-->B\n",
        "```\n\n",
        "after\n",
    );
    let output = run_with_stdin(
        &[
            "fix",
            "--markdown",
            "--disable-rule",
            FRONTMATTER_RULE,
            "--rule",
            INIT_RULE,
            "-",
        ],
        source,
    );

    assert_eq!(exit_code(output.status), 0, "stderr:\n{}", stderr(&output));
    let fixed = stdout(&output);
    assert!(fixed.starts_with("before\n\n```mermaid\n"), "{fixed}");
    assert!(fixed.contains("%%{ init:"), "{fixed}");
    assert!(fixed.contains("中文"), "{fixed}");
    assert!(fixed.ends_with("```\n\nafter\n"), "{fixed}");
}

#[test]
fn primary_input_failures_do_not_create_output() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("input.mmd"), "flowchart\nA-->B\n").expect("write source");

    let output = Command::new(cargo_bin("merman-cli"))
        .current_dir(tmp.path())
        .args([
            "fix",
            "input.mmd",
            "--output",
            "fixed.mmd",
            "--resource-limit",
            "max_source_bytes=1",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("run limited fix");

    assert_eq!(exit_code(output.status), 1, "stderr:\n{}", stderr(&output));
    assert!(!tmp.path().join("fixed.mmd").exists());

    let invalid_path = tmp.path().join("invalid-fixed.mmd");
    let invalid_path_arg = invalid_path.to_string_lossy();
    let invalid = run_with_stdin_bytes(
        &["fix", "--output", invalid_path_arg.as_ref(), "-"],
        &[0xff],
    );
    assert_eq!(
        exit_code(invalid.status),
        1,
        "stderr:\n{}",
        stderr(&invalid)
    );
    assert!(!invalid_path.exists());
}
